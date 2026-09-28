//! Resumable, verified downloads from official sources.
//!
//! - Partial data is kept in `<dest>.part` next to a small `.part.json` sidecar that records the
//!   URL, ETag/Last-Modified and expected size, so an interrupted download resumes with a
//!   `Range` request and restarts if the server's file changed.
//! - A download is only reported as `verified` when its SHA-256 matched an expected value.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use crate::cmd::tokio_util_lite::CancellationToken;
use crate::host::{Arch, HostOs};
use crate::verify;
use crate::{CoreError, Result};

pub const USER_AGENT: &str = concat!("VMSetupAssistant/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DownloadSpec {
    pub display_name: String,
    /// Human-readable official source (shown in the UI), e.g. "download.virtualbox.org".
    pub source_label: String,
    pub url: String,
    pub dest: PathBuf,
    pub expected_sha256: Option<String>,
    pub expected_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DownloadProgress {
    pub bytes_done: u64,
    /// None when the server does not report a length.
    pub bytes_total: Option<u64>,
    pub bytes_per_sec: f64,
    pub resumed_from: u64,
    pub phase: DownloadPhase,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadPhase {
    Connecting,
    Downloading,
    Verifying,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DownloadResult {
    pub path: PathBuf,
    pub size: u64,
    pub sha256: String,
    /// True only when `expected_sha256` was provided and matched.
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PartMeta {
    url: String,
    etag: Option<String>,
    last_modified: Option<String>,
    total: Option<u64>,
}

pub fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(30))
        .read_timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| CoreError::Download(e.to_string()))
}

fn part_paths(dest: &Path) -> (PathBuf, PathBuf) {
    let mut part = dest.as_os_str().to_owned();
    part.push(".part");
    let mut meta = part.clone();
    meta.push(".json");
    (PathBuf::from(part), PathBuf::from(meta))
}

/// Download `spec` with resume support. If the destination already exists and verifies, it is
/// returned without touching the network (idempotent retries).
pub async fn download(
    client: &reqwest::Client,
    spec: &DownloadSpec,
    mut progress: impl FnMut(DownloadProgress),
    cancel: &CancellationToken,
) -> Result<DownloadResult> {
    if let Some(dir) = spec.dest.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| CoreError::io(dir, e))?;
    }

    // Already complete?
    if spec.dest.is_file() {
        progress(DownloadProgress {
            bytes_done: 0,
            bytes_total: None,
            bytes_per_sec: 0.0,
            resumed_from: 0,
            phase: DownloadPhase::Verifying,
        });
        match finish_verify(spec, cancel).await {
            Ok(r) => return Ok(r),
            Err(CoreError::ChecksumMismatch { .. }) => {
                // Stale or corrupt file: remove and download again.
                let _ = tokio::fs::remove_file(&spec.dest).await;
            }
            Err(e) => return Err(e),
        }
    }

    let (part, meta_path) = part_paths(&spec.dest);
    let mut resume_from = 0u64;
    let mut meta: PartMeta = match tokio::fs::read(&meta_path).await {
        Ok(b) => serde_json::from_slice(&b).unwrap_or_default(),
        Err(_) => PartMeta::default(),
    };
    if part.is_file() && meta.url == spec.url {
        resume_from = tokio::fs::metadata(&part)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
    } else {
        let _ = tokio::fs::remove_file(&part).await;
        meta = PartMeta {
            url: spec.url.clone(),
            ..Default::default()
        };
    }

    progress(DownloadProgress {
        bytes_done: resume_from,
        bytes_total: meta.total,
        bytes_per_sec: 0.0,
        resumed_from: resume_from,
        phase: DownloadPhase::Connecting,
    });

    let mut req = client.get(&spec.url);
    if resume_from > 0 {
        req = req.header(reqwest::header::RANGE, format!("bytes={resume_from}-"));
        if let Some(etag) = &meta.etag {
            req = req.header(reqwest::header::IF_RANGE, etag.clone());
        } else if let Some(lm) = &meta.last_modified {
            req = req.header(reqwest::header::IF_RANGE, lm.clone());
        }
    }
    let resp = tokio::select! {
        r = req.send() => r.map_err(|e| CoreError::Download(format!("{}: {}", spec.display_name, describe_reqwest(&e))))?,
        _ = cancel.cancelled() => return Err(CoreError::Cancelled),
    };
    let status = resp.status();
    if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        // The part file may already be complete; try verifying it.
        tokio::fs::rename(&part, &spec.dest)
            .await
            .map_err(|e| CoreError::io(&part, e))?;
        let _ = tokio::fs::remove_file(&meta_path).await;
        return finish_verify(spec, cancel).await;
    }
    if !status.is_success() {
        return Err(CoreError::Download(format!(
            "{}: the server answered {} for {}",
            spec.display_name, status, spec.source_label
        )));
    }
    let resumed = status == reqwest::StatusCode::PARTIAL_CONTENT && resume_from > 0;
    if !resumed {
        resume_from = 0;
    }
    let headers = resp.headers();
    let content_len = resp.content_length();
    let total = if resumed {
        // Content-Range: bytes start-end/total
        headers
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.rsplit('/').next())
            .and_then(|t| t.parse::<u64>().ok())
            .or(meta.total)
    } else {
        content_len
    };
    meta.etag = headers
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    meta.last_modified = headers
        .get(reqwest::header::LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    meta.total = total;
    if let Ok(b) = serde_json::to_vec(&meta) {
        let _ = tokio::fs::write(&meta_path, b).await;
    }

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(resumed)
        .truncate(!resumed)
        .open(&part)
        .await
        .map_err(|e| CoreError::io(&part, e))?;

    let mut done = resume_from;
    let started = Instant::now();
    let mut last_emit = Instant::now();
    let mut window_start = Instant::now();
    let mut window_bytes = 0u64;
    let mut rate = 0.0;
    let mut stream = resp.bytes_stream();
    loop {
        let chunk = tokio::select! {
            c = stream.next() => c,
            _ = cancel.cancelled() => {
                let _ = file.flush().await;
                return Err(CoreError::Cancelled);
            }
        };
        let Some(chunk) = chunk else { break };
        let chunk = chunk.map_err(|e| {
            CoreError::Download(format!(
                "{}: connection interrupted ({})",
                spec.display_name,
                describe_reqwest(&e)
            ))
        })?;
        file.write_all(&chunk)
            .await
            .map_err(|e| CoreError::io(&part, e))?;
        done += chunk.len() as u64;
        window_bytes += chunk.len() as u64;
        if window_start.elapsed() >= Duration::from_secs(1) {
            rate = window_bytes as f64 / window_start.elapsed().as_secs_f64();
            window_start = Instant::now();
            window_bytes = 0;
        }
        if last_emit.elapsed() >= Duration::from_millis(250) {
            last_emit = Instant::now();
            progress(DownloadProgress {
                bytes_done: done,
                bytes_total: total,
                bytes_per_sec: rate,
                resumed_from: resume_from,
                phase: DownloadPhase::Downloading,
            });
        }
    }
    file.flush().await.map_err(|e| CoreError::io(&part, e))?;
    drop(file);
    let _ = started;

    if let Some(t) = total {
        if done != t {
            return Err(CoreError::Download(format!(
                "{}: connection closed early ({done} of {t} bytes). Retry to resume.",
                spec.display_name
            )));
        }
    }
    if let Some(expected) = spec.expected_size {
        if done != expected {
            let _ = tokio::fs::remove_file(&part).await;
            let _ = tokio::fs::remove_file(&meta_path).await;
            return Err(CoreError::Download(format!(
                "{}: unexpected size {done} (expected {expected})",
                spec.display_name
            )));
        }
    }
    tokio::fs::rename(&part, &spec.dest)
        .await
        .map_err(|e| CoreError::io(&part, e))?;
    let _ = tokio::fs::remove_file(&meta_path).await;
    progress(DownloadProgress {
        bytes_done: done,
        bytes_total: total,
        bytes_per_sec: rate,
        resumed_from: resume_from,
        phase: DownloadPhase::Verifying,
    });
    finish_verify(spec, cancel).await
}

async fn finish_verify(spec: &DownloadSpec, cancel: &CancellationToken) -> Result<DownloadResult> {
    let size = tokio::fs::metadata(&spec.dest)
        .await
        .map_err(|e| CoreError::io(&spec.dest, e))?
        .len();
    let sha256 = verify::sha256_file(&spec.dest, None, Some(cancel)).await?;
    let verified = match &spec.expected_sha256 {
        Some(exp) => {
            if !exp.eq_ignore_ascii_case(&sha256) {
                let _ = tokio::fs::remove_file(&spec.dest).await;
                return Err(CoreError::ChecksumMismatch {
                    file: spec.dest.display().to_string(),
                    expected: exp.to_ascii_lowercase(),
                    actual: sha256,
                });
            }
            true
        }
        None => false,
    };
    Ok(DownloadResult {
        path: spec.dest.clone(),
        size,
        sha256,
        verified,
    })
}

fn describe_reqwest(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "timed out".into()
    } else if e.is_connect() {
        "could not connect (is this computer online?)".into()
    } else {
        e.to_string()
    }
}

// ---------- VirtualBox release manifest ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VirtualBoxRelease {
    pub version: String,
    pub file_name: String,
    pub url: String,
    pub sha256: String,
    /// Where the checksum came from.
    pub checksum_source: ChecksumSource,
    pub sha256sums_url: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChecksumSource {
    /// Fetched live from Oracle's published SHA256SUMS for that version.
    OracleSha256Sums,
    /// Built into this app release as a fallback when virtualbox.org is unreachable.
    PinnedFallback,
}

pub const VBOX_LATEST_URL: &str = "https://download.virtualbox.org/virtualbox/LATEST-STABLE.TXT";
pub fn vbox_sha256sums_url(version: &str) -> String {
    format!("https://www.virtualbox.org/download/hashes/{version}/SHA256SUMS")
}
pub fn vbox_file_url(version: &str, file: &str) -> String {
    format!("https://download.virtualbox.org/virtualbox/{version}/{file}")
}

/// Pinned fallback for VirtualBox 7.2.20 (build 175154, released 2026-09-22).
/// Provenance: Windows checksum from the winget community manifest `Oracle.VirtualBox 7.2.20`,
/// macOS checksums from the Homebrew cask `virtualbox 7.2.20,175154`. Both mirror Oracle's
/// published SHA256SUMS; the live file is preferred whenever reachable.
pub fn pinned_fallback(os: HostOs, arch: Arch) -> Option<VirtualBoxRelease> {
    let version = "7.2.20";
    let (file, sha) = match (os, arch) {
        (HostOs::Windows, Arch::X86_64) => (
            "VirtualBox-7.2.20-175154-Win.exe",
            "a81777d2b36380ce042a29e9c554cf032eb46a793f62e3cc82e7411e535c2c26",
        ),
        (HostOs::MacOs, Arch::Aarch64) => (
            "VirtualBox-7.2.20-175154-macOSArm64.dmg",
            "186eb4734234bcf20bc71c577060045507aa0769912e7664735ad18cd7458d8d",
        ),
        (HostOs::MacOs, Arch::X86_64) => (
            "VirtualBox-7.2.20-175154-OSX.dmg",
            "8d171af268b08b3416978d09fd2d6a22866962172891d02b906ad2a87e55752f",
        ),
        _ => return None,
    };
    Some(VirtualBoxRelease {
        version: version.into(),
        file_name: file.into(),
        url: vbox_file_url(version, file),
        sha256: sha.into(),
        checksum_source: ChecksumSource::PinnedFallback,
        sha256sums_url: vbox_sha256sums_url(version),
    })
}

/// File-name suffix Oracle uses for each host package.
pub fn vbox_package_suffix(os: HostOs, arch: Arch) -> Option<&'static str> {
    match (os, arch) {
        (HostOs::Windows, Arch::X86_64) | (HostOs::Windows, Arch::Aarch64) => Some("-Win.exe"),
        (HostOs::MacOs, Arch::Aarch64) => Some("-macOSArm64.dmg"),
        (HostOs::MacOs, Arch::X86_64) => Some("-OSX.dmg"),
        _ => None,
    }
}

/// Choose the host package from an Oracle SHA256SUMS listing (pure, testable).
pub fn select_release(
    version: &str,
    sums: &str,
    os: HostOs,
    arch: Arch,
) -> Option<VirtualBoxRelease> {
    let suffix = vbox_package_suffix(os, arch)?;
    let prefix = format!("VirtualBox-{version}-");
    let entries = verify::parse_sha256sums(sums);
    let e = entries
        .iter()
        .find(|e| e.file_name.starts_with(&prefix) && e.file_name.ends_with(suffix))?;
    Some(VirtualBoxRelease {
        version: version.into(),
        file_name: e.file_name.clone(),
        url: vbox_file_url(version, &e.file_name),
        sha256: e.sha256.clone(),
        checksum_source: ChecksumSource::OracleSha256Sums,
        sha256sums_url: vbox_sha256sums_url(version),
    })
}

/// Resolve the current VirtualBox package for this host from Oracle's own metadata, falling
/// back to the pinned release when the site is unreachable (the fallback is labelled as such).
pub async fn resolve_virtualbox_release(
    client: &reqwest::Client,
    os: HostOs,
    arch: Arch,
) -> Result<VirtualBoxRelease> {
    let live = async {
        let version = client
            .get(VBOX_LATEST_URL)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let version = version.trim().to_string();
        let sums = client
            .get(vbox_sha256sums_url(&version))
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        Ok::<_, reqwest::Error>((version, sums))
    };
    match tokio::time::timeout(Duration::from_secs(45), live).await {
        Ok(Ok((version, sums))) => {
            if let Some(r) = select_release(&version, &sums, os, arch) {
                return Ok(r);
            }
            tracing::warn!(
                version,
                "no matching package in SHA256SUMS; using pinned fallback"
            );
        }
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "could not reach virtualbox.org; using pinned fallback")
        }
        Err(_) => tracing::warn!("virtualbox.org metadata timed out; using pinned fallback"),
    }
    pinned_fallback(os, arch).ok_or_else(|| {
        CoreError::Unsupported("No VirtualBox package is known for this computer type.".into())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUMS: &str = "\
a81777d2b36380ce042a29e9c554cf032eb46a793f62e3cc82e7411e535c2c26 *VirtualBox-7.2.20-175154-Win.exe
186eb4734234bcf20bc71c577060045507aa0769912e7664735ad18cd7458d8d *VirtualBox-7.2.20-175154-macOSArm64.dmg
8d171af268b08b3416978d09fd2d6a22866962172891d02b906ad2a87e55752f *VirtualBox-7.2.20-175154-OSX.dmg
0000000000000000000000000000000000000000000000000000000000000000 *Oracle_VirtualBox_Extension_Pack-7.2.20.vbox-extpack
1111111111111111111111111111111111111111111111111111111111111111 *VBoxGuestAdditions_7.2.20.iso
";

    #[test]
    fn selects_correct_package_per_host() {
        let w = select_release("7.2.20", SUMS, HostOs::Windows, Arch::X86_64).unwrap();
        assert_eq!(w.file_name, "VirtualBox-7.2.20-175154-Win.exe");
        assert_eq!(
            w.url,
            "https://download.virtualbox.org/virtualbox/7.2.20/VirtualBox-7.2.20-175154-Win.exe"
        );
        assert_eq!(w.checksum_source, ChecksumSource::OracleSha256Sums);
        let m = select_release("7.2.20", SUMS, HostOs::MacOs, Arch::Aarch64).unwrap();
        assert!(m.file_name.ends_with("macOSArm64.dmg"));
        let i = select_release("7.2.20", SUMS, HostOs::MacOs, Arch::X86_64).unwrap();
        assert!(i.file_name.ends_with("-OSX.dmg"));
        assert!(select_release("7.2.20", SUMS, HostOs::Linux, Arch::X86_64).is_none());
        assert!(select_release("7.2.21", SUMS, HostOs::Windows, Arch::X86_64).is_none());
    }

    #[test]
    fn pinned_fallback_matches_live_format() {
        let p = pinned_fallback(HostOs::MacOs, Arch::Aarch64).unwrap();
        let live = select_release("7.2.20", SUMS, HostOs::MacOs, Arch::Aarch64).unwrap();
        assert_eq!(p.file_name, live.file_name);
        assert_eq!(p.sha256, live.sha256);
        assert_eq!(p.checksum_source, ChecksumSource::PinnedFallback);
        assert!(pinned_fallback(HostOs::Windows, Arch::Aarch64).is_none());
    }

    #[test]
    fn part_paths_are_siblings() {
        let (p, m) = part_paths(Path::new("/x/Win11 (1).iso"));
        assert_eq!(p, PathBuf::from("/x/Win11 (1).iso.part"));
        assert_eq!(m, PathBuf::from("/x/Win11 (1).iso.part.json"));
    }

    #[tokio::test]
    async fn existing_verified_file_is_not_redownloaded() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("hello.bin");
        std::fs::write(&dest, b"hello").unwrap();
        let spec = DownloadSpec {
            display_name: "hello".into(),
            source_label: "test".into(),
            url: "http://127.0.0.1:9/unreachable".into(),
            dest: dest.clone(),
            expected_sha256: Some(
                "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".into(),
            ),
            expected_size: None,
        };
        let client = http_client().unwrap();
        let r = download(&client, &spec, |_| {}, &CancellationToken::new())
            .await
            .unwrap();
        assert!(r.verified);
        assert_eq!(r.size, 5);
    }

    #[tokio::test]
    async fn corrupt_existing_file_fails_verification_and_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("hello.bin");
        std::fs::write(&dest, b"hellx").unwrap();
        let spec = DownloadSpec {
            display_name: "hello".into(),
            source_label: "test".into(),
            url: "http://127.0.0.1:9/unreachable".into(),
            dest: dest.clone(),
            expected_sha256: Some(
                "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".into(),
            ),
            expected_size: None,
        };
        let client = http_client().unwrap();
        // The corrupt file is discarded and a download is attempted, which fails (unreachable).
        let err = download(&client, &spec, |_| {}, &CancellationToken::new())
            .await
            .unwrap_err();
        assert!(matches!(err, CoreError::Download(_)), "{err:?}");
        assert!(!dest.exists());
    }
}
