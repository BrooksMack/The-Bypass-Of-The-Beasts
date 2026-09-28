//! User-appropriate directories. Large VM disks never live inside the app bundle,
//! the repository, a temp directory, or (by default) a cloud-synced Documents folder.

use std::path::PathBuf;

use directories::{BaseDirs, ProjectDirs};

pub const APP_QUALIFIER: &str = "org";
pub const APP_ORG: &str = "vm-setup-assistant";
pub const APP_NAME: &str = "VM Setup Assistant";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppPaths {
    /// Persistent state (setup-state.json, ownership records).
    pub data_dir: PathBuf,
    /// Downloaded installers and ISOs (large, resumable).
    pub downloads_dir: PathBuf,
    /// Application logs (redacted support reports are generated from these).
    pub logs_dir: PathBuf,
    /// Default base folder for VM files (VirtualBox's own default, "VirtualBox VMs" in the home folder).
    pub default_vm_base: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Option<Self> {
        let proj = ProjectDirs::from(APP_QUALIFIER, APP_ORG, APP_NAME)?;
        let base = BaseDirs::new()?;
        let data_dir = proj.data_local_dir().to_path_buf();
        let downloads_dir = data_dir.join("downloads");
        let logs_dir = data_dir.join("logs");
        // VirtualBox's default machine folder. It is outside Documents on every platform,
        // so it is not normally cloud-synced. Users can override it in Advanced settings.
        let default_vm_base = base.home_dir().join("VirtualBox VMs");
        Some(Self {
            data_dir,
            downloads_dir,
            logs_dir,
            default_vm_base,
        })
    }

    pub fn ensure(&self) -> std::io::Result<()> {
        for d in [&self.data_dir, &self.downloads_dir, &self.logs_dir] {
            std::fs::create_dir_all(d)?;
        }
        Ok(())
    }

    pub fn state_file(&self) -> PathBuf {
        self.data_dir.join("setup-state.json")
    }

    pub fn lock_file(&self) -> PathBuf {
        self.data_dir.join("setup.lock")
    }
}

/// Heuristic: does this path look like it is inside a cloud-synced folder?
/// Used to warn (not block) when a user picks such a location for a 100 GB disk.
pub fn looks_cloud_synced(path: &std::path::Path) -> bool {
    let s = path.to_string_lossy().to_ascii_lowercase();
    [
        "onedrive",
        "icloud",
        "dropbox",
        "google drive",
        "googledrive",
        "mobile documents",
        "box sync",
    ]
    .iter()
    .any(|k| s.contains(k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_heuristic() {
        assert!(looks_cloud_synced(std::path::Path::new(
            "C:\\Users\\a\\OneDrive\\VMs"
        )));
        assert!(looks_cloud_synced(std::path::Path::new(
            "/Users/a/Library/Mobile Documents/com~apple~CloudDocs"
        )));
        assert!(!looks_cloud_synced(std::path::Path::new(
            "/Users/a/VirtualBox VMs"
        )));
    }
}
