//! VirtualBox installer handoff. The official installer runs interactively (it shows the OS
//! permission prompt itself); this module launches it, waits, and interprets the result.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::cmd::{self, tokio_util_lite::CancellationToken};
use crate::{CoreError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstallOutcome {
    Succeeded,
    RebootRequired,
    Cancelled,
    AnotherInstallRunning,
    Failed {
        code: Option<i32>,
        message: String,
    },
    /// The installer ran but the app could not tell what happened; detection decides.
    Unknown,
}

/// Interpret Windows Installer / VirtualBox bootstrapper exit codes.
pub fn interpret_windows_exit(code: Option<i32>) -> InstallOutcome {
    match code {
        Some(0) => InstallOutcome::Succeeded,
        Some(3010) | Some(1641) => InstallOutcome::RebootRequired,
        Some(1602) | Some(1223) => InstallOutcome::Cancelled,
        Some(1618) => InstallOutcome::AnotherInstallRunning,
        Some(1603) => InstallOutcome::Failed { code, message: "The installer reported a fatal error (1603). Restarting Windows and running the installer again usually fixes this.".into() },
        Some(1633) => InstallOutcome::Failed { code, message: "This installer is for a different processor type.".into() },
        Some(c) => InstallOutcome::Failed { code: Some(c), message: format!("The installer exited with code {c}.") },
        None => InstallOutcome::Unknown,
    }
}

/// Windows: run the official `VirtualBox-<ver>-<rev>-Win.exe`. It elevates via UAC by itself.
pub async fn run_windows_installer(
    installer: &Path,
    cancel: Option<&CancellationToken>,
) -> Result<InstallOutcome> {
    // Interactive install, but suppress the automatic post-install start of the VirtualBox GUI.
    let args = vec!["-msiparams".to_string(), "VBOX_START=0".to_string()];
    let out = cmd::run(installer, &args, Duration::from_secs(60 * 30), cancel, None).await?;
    Ok(interpret_windows_exit(out.code))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MountedDmg {
    pub mount_point: PathBuf,
    pub pkg_path: PathBuf,
}

/// Parse `hdiutil attach -plist` output for the mount point.
pub fn parse_hdiutil_mount_point(plist: &str) -> Option<PathBuf> {
    // <key>mount-point</key>\n<string>/Volumes/VirtualBox</string>
    let idx = plist.find("<key>mount-point</key>")?;
    let rest = &plist[idx..];
    let start = rest.find("<string>")? + "<string>".len();
    let end = rest[start..].find("</string>")? + start;
    Some(PathBuf::from(rest[start..end].trim()))
}

/// macOS: mount the DMG (no Finder window) and locate `VirtualBox.pkg`.
pub async fn mount_dmg(dmg: &Path) -> Result<MountedDmg> {
    let args = vec![
        "attach".to_string(),
        "-nobrowse".into(),
        "-plist".into(),
        dmg.to_string_lossy().into_owned(),
    ];
    let out = cmd::run(
        Path::new("/usr/bin/hdiutil"),
        &args,
        Duration::from_secs(120),
        None,
        None,
    )
    .await?;
    if !out.success() {
        return Err(CoreError::Other(format!(
            "Could not open the VirtualBox disk image: {}",
            out.stderr.trim()
        )));
    }
    let mount_point = parse_hdiutil_mount_point(&out.stdout)
        .ok_or_else(|| CoreError::Parse("hdiutil output had no mount point".into()))?;
    let pkg_path = mount_point.join("VirtualBox.pkg");
    if !pkg_path.exists() {
        return Err(CoreError::Other(
            "The disk image does not contain VirtualBox.pkg.".into(),
        ));
    }
    Ok(MountedDmg {
        mount_point,
        pkg_path,
    })
}

pub async fn unmount_dmg(mount_point: &Path) -> Result<()> {
    let args = vec![
        "detach".to_string(),
        mount_point.to_string_lossy().into_owned(),
        "-quiet".into(),
    ];
    let _ = cmd::run(
        Path::new("/usr/bin/hdiutil"),
        &args,
        Duration::from_secs(60),
        None,
        None,
    )
    .await;
    Ok(())
}

/// macOS: open the package in Installer.app. The user completes the guided installer, which
/// asks for an administrator password in the system dialog. This app never sees that password.
pub async fn open_pkg_in_installer(pkg: &Path) -> Result<()> {
    let args = vec!["-W".to_string(), pkg.to_string_lossy().into_owned()];
    // `open -W` waits until Installer.app quits; give it up to 30 minutes.
    let out = cmd::run(
        Path::new("/usr/bin/open"),
        &args,
        Duration::from_secs(60 * 30),
        None,
        None,
    )
    .await?;
    if !out.success() {
        return Err(CoreError::Other(format!(
            "Could not open the installer: {}",
            out.stderr.trim()
        )));
    }
    Ok(())
}

/// Wait for a detection closure to report an installed VirtualBox, polling with a deadline.
pub async fn wait_for_install<F, Fut>(
    mut check: F,
    deadline: Duration,
    cancel: Option<&CancellationToken>,
) -> Result<bool>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = std::time::Instant::now();
    while start.elapsed() < deadline {
        if let Some(c) = cancel {
            if c.is_cancelled() {
                return Err(CoreError::Cancelled);
            }
        }
        if check().await {
            return Ok(true);
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes() {
        assert_eq!(interpret_windows_exit(Some(0)), InstallOutcome::Succeeded);
        assert_eq!(
            interpret_windows_exit(Some(3010)),
            InstallOutcome::RebootRequired
        );
        assert_eq!(
            interpret_windows_exit(Some(1641)),
            InstallOutcome::RebootRequired
        );
        assert_eq!(
            interpret_windows_exit(Some(1602)),
            InstallOutcome::Cancelled
        );
        assert_eq!(
            interpret_windows_exit(Some(1223)),
            InstallOutcome::Cancelled
        );
        assert_eq!(
            interpret_windows_exit(Some(1618)),
            InstallOutcome::AnotherInstallRunning
        );
        assert!(matches!(
            interpret_windows_exit(Some(1603)),
            InstallOutcome::Failed {
                code: Some(1603),
                ..
            }
        ));
        assert_eq!(interpret_windows_exit(None), InstallOutcome::Unknown);
    }

    #[test]
    fn hdiutil_plist() {
        let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict><key>system-entities</key><array>
<dict><key>content-hint</key><string>GUID_partition_scheme</string><key>dev-entry</key><string>/dev/disk4</string></dict>
<dict><key>content-hint</key><string>Apple_HFS</string><key>dev-entry</key><string>/dev/disk4s2</string>
<key>mount-point</key>
<string>/Volumes/VirtualBox</string></dict></array></dict></plist>"#;
        assert_eq!(
            parse_hdiutil_mount_point(plist),
            Some(PathBuf::from("/Volumes/VirtualBox"))
        );
        assert!(parse_hdiutil_mount_point("nope").is_none());
    }

    #[tokio::test]
    async fn wait_for_install_polls_until_true() {
        let mut n = 0;
        let ok = wait_for_install(
            || {
                n += 1;
                let v = n >= 2;
                async move { v }
            },
            Duration::from_secs(30),
            None,
        )
        .await
        .unwrap();
        assert!(ok);
    }
}
