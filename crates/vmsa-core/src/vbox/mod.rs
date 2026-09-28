//! VirtualBox integration through `VBoxManage` (Oracle's supported command-line interface).
//! No GUI automation, no shell interpolation: every call is an argument array.

pub mod client;
pub mod parse;
pub mod plan;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::host::HostOs;
use crate::profile::VboxVersion;

/// Extra-data keys the app writes to mark VMs it created (ownership survives state-file loss).
pub const EXTRADATA_CREATED_BY: &str = "VMSetupAssistant/CreatedBy";
pub const EXTRADATA_INSTANCE_ID: &str = "VMSetupAssistant/InstanceId";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VirtualBoxInstall {
    pub vboxmanage: PathBuf,
    pub install_dir: PathBuf,
    pub version: VboxVersion,
    pub version_raw: String,
    pub guest_additions_iso: Option<PathBuf>,
}

/// Candidate VBoxManage locations per host OS (checked before PATH).
pub fn vboxmanage_candidates(os: HostOs) -> Vec<PathBuf> {
    match os {
        HostOs::Windows => {
            let mut v = Vec::new();
            #[cfg(windows)]
            {
                if let Some(dir) = windows_install_dir_from_registry() {
                    v.push(dir.join("VBoxManage.exe"));
                }
            }
            if let Some(pf) = std::env::var_os("ProgramFiles") {
                v.push(PathBuf::from(pf).join("Oracle").join("VirtualBox").join("VBoxManage.exe"));
            }
            if let Some(vbox) = std::env::var_os("VBOX_MSI_INSTALL_PATH").or_else(|| std::env::var_os("VBOX_INSTALL_PATH")) {
                v.push(PathBuf::from(vbox).join("VBoxManage.exe"));
            }
            v.push(PathBuf::from(r"C:\Program Files\Oracle\VirtualBox\VBoxManage.exe"));
            v
        }
        HostOs::MacOs => vec![
            PathBuf::from("/Applications/VirtualBox.app/Contents/MacOS/VBoxManage"),
            PathBuf::from("/usr/local/bin/VBoxManage"),
        ],
        _ => vec![PathBuf::from("/usr/bin/VBoxManage"), PathBuf::from("/usr/lib/virtualbox/VBoxManage")],
    }
}

#[cfg(windows)]
fn windows_install_dir_from_registry() -> Option<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm.open_subkey(r"SOFTWARE\Oracle\VirtualBox").ok()?;
    let dir: String = key.get_value("InstallDir").ok()?;
    Some(PathBuf::from(dir))
}

/// Where VirtualBox keeps its Guest Additions ISO, per platform.
pub fn guest_additions_iso_candidates(os: HostOs, install_dir: &Path) -> Vec<PathBuf> {
    match os {
        HostOs::Windows => vec![install_dir.join("VBoxGuestAdditions.iso")],
        HostOs::MacOs => vec![
            install_dir.join("VBoxGuestAdditions.iso"),
            PathBuf::from("/Applications/VirtualBox.app/Contents/MacOS/VBoxGuestAdditions.iso"),
        ],
        _ => vec![
            PathBuf::from("/usr/share/virtualbox/VBoxGuestAdditions.iso"),
            PathBuf::from("/opt/VirtualBox/additions/VBoxGuestAdditions.iso"),
        ],
    }
}

/// Derive the install dir from the VBoxManage path (its parent directory).
pub fn install_dir_of(vboxmanage: &Path) -> PathBuf {
    vboxmanage.parent().map(|p| p.to_path_buf()).unwrap_or_default()
}
