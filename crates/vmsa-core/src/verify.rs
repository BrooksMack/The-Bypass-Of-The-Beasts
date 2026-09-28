//! SHA-256 verification and checksum-file parsing. A file is never described as verified
//! unless its digest was computed and matched an expected value from an official source.

use std::path::Path;

use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;

use crate::{CoreError, Result};

/// Compute the SHA-256 of a file, streaming, with optional progress and cancellation.
pub async fn sha256_file(
    path: &Path,
    mut progress: Option<&mut dyn FnMut(u64, u64)>,
    cancel: Option<&crate::cmd::tokio_util_lite::CancellationToken>,
) -> Result<String> {
    let mut f = tokio::fs::File::open(path).await.map_err(|e| CoreError::io(path, e))?;
    let total = f.metadata().await.map_err(|e| CoreError::io(path, e))?.len();
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    let mut done = 0u64;
    loop {
        if let Some(c) = cancel {
            if c.is_cancelled() {
                return Err(CoreError::Cancelled);
            }
        }
        let n = f.read(&mut buf).await.map_err(|e| CoreError::io(path, e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        done += n as u64;
        if let Some(p) = progress.as_deref_mut() {
            p(done, total);
        }
    }
    Ok(hex::encode(hasher.finalize()))
}

/// One entry from a `SHA256SUMS`-style file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecksumEntry {
    pub sha256: String,
    pub file_name: String,
}

/// Parse `sha256sum`-style text: `<hex>  <name>` or `<hex> *<name>`. Lines that do not fit are ignored.
pub fn parse_sha256sums(text: &str) -> Vec<ChecksumEntry> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (hash, rest) = line.split_once(char::is_whitespace)?;
            if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
                return None;
            }
            let name = rest.trim().trim_start_matches('*').trim();
            // Entries may be prefixed with a path; keep only the file name.
            let name = name.rsplit(['/', '\\']).next().unwrap_or(name);
            Some(ChecksumEntry {
                sha256: hash.to_ascii_lowercase(),
                file_name: name.to_string(),
            })
        })
        .collect()
}

pub fn find_checksum<'a>(entries: &'a [ChecksumEntry], file_name: &str) -> Option<&'a ChecksumEntry> {
    entries.iter().find(|e| e.file_name.eq_ignore_ascii_case(file_name))
}

pub fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_oracle_style_sums() {
        let text = "a81777d2b36380ce042a29e9c554cf032eb46a793f62e3cc82e7411e535c2c26 *VirtualBox-7.2.20-175154-Win.exe\n\
                    186eb4734234bcf20bc71c577060045507aa0769912e7664735ad18cd7458d8d  VirtualBox-7.2.20-175154-macOSArm64.dmg\n\
                    junk line\n\
                    abc *short.txt\n";
        let e = parse_sha256sums(text);
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].file_name, "VirtualBox-7.2.20-175154-Win.exe");
        assert_eq!(find_checksum(&e, "virtualbox-7.2.20-175154-macosarm64.dmg").unwrap().sha256.len(), 64);
        assert!(find_checksum(&e, "nope").is_none());
    }

    #[tokio::test]
    async fn hashes_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("hello.txt");
        std::fs::write(&p, b"hello").unwrap();
        let h = sha256_file(&p, None, None).await.unwrap();
        assert_eq!(h, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
    }
}
