//! Pure construction of VBoxManage command plans. Nothing here runs a process, which keeps
//! the exact arguments testable (spaces, Unicode, braces in names are passed verbatim).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::profile::{GuestArch, VmProfile, VmSizing};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VboxCommand {
    /// Short, user-facing description of this step.
    pub description: String,
    pub args: Vec<String>,
    /// Timeout in seconds.
    pub timeout_secs: u64,
}

fn cmd(description: &str, args: &[&str], timeout_secs: u64) -> VboxCommand {
    VboxCommand {
        description: description.into(),
        args: args.iter().map(|s| s.to_string()).collect(),
        timeout_secs,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateVmSpec {
    pub name: String,
    pub base_folder: PathBuf,
    pub sizing: VmSizing,
    pub install_iso: PathBuf,
    pub guest_additions_iso: Option<PathBuf>,
    pub profile: VmProfile,
    /// Opaque id written to the VM's extra data so ownership survives state loss.
    pub instance_id: String,
    pub app_version: String,
}

/// Step 1: create and register the VM with Oracle's defaults for the OS type.
pub fn createvm(spec: &CreateVmSpec) -> VboxCommand {
    cmd(
        "Register the new virtual machine",
        &[
            "createvm",
            "--name",
            &spec.name,
            "--platform-architecture",
            &spec.profile.platform_architecture,
            "--ostype",
            &spec.profile.ostype_id,
            "--basefolder",
            &spec.base_folder.to_string_lossy(),
            "--default",
            "--register",
        ],
        120,
    )
}

/// Steps after the VM exists. `existing_controllers` comes from `showvminfo` after `--default`,
/// because Oracle's defaults may already have added a storage controller.
pub fn configure(
    spec: &CreateVmSpec,
    uuid: &str,
    existing_controllers: &[String],
    disk_path: &Path,
) -> Vec<VboxCommand> {
    let p = &spec.profile;
    let mut plan = Vec::new();

    let ram = spec.sizing.ram_mb.to_string();
    let cpus = spec.sizing.cpus.to_string();
    let vram = p.vram_mb.to_string();
    let mut modify: Vec<&str> = vec![
        "modifyvm",
        uuid,
        "--memory",
        &ram,
        "--cpus",
        &cpus,
        "--vram",
        &vram,
        "--graphicscontroller",
        &p.graphics_controller,
        "--nic1",
        "nat",
        "--nic-type1",
        &p.nic_type,
        "--cable-connected1",
        "on",
        "--usb-xhci",
        "on",
        "--mouse",
        "usbtablet",
        "--audio-driver",
        "default",
        "--audio-enabled",
        "on",
        "--audio-out",
        "on",
    ];
    match p.guest_arch {
        GuestArch::X64 => {
            modify.extend([
                "--firmware",
                &p.firmware,
                "--chipset",
                &p.chipset,
                "--boot1",
                "dvd",
                "--boot2",
                "disk",
                "--boot3",
                "none",
                "--boot4",
                "none",
            ]);
            if let Some(t) = &p.tpm_type {
                modify.extend(["--tpm-type", t.as_str()]);
            }
        }
        GuestArch::Arm64 => {
            // Arm VMs are always EFI; boot order and TPM settings are not exposed for Arm guests.
            modify.extend(["--rtc-use-utc", "on"]);
        }
    }
    plan.push(cmd(
        "Apply memory, processor, graphics and network settings",
        &modify,
        120,
    ));

    plan.push(cmd(
        "Disable host clipboard and drag-and-drop sharing",
        &[
            "modifyvm",
            uuid,
            "--clipboard-mode",
            "disabled",
            "--clipboard-file-transfers",
            "disabled",
            "--drag-and-drop",
            "disabled",
        ],
        120,
    ));

    let size_mb = (spec.sizing.disk_gb as u64 * 1024).to_string();
    let disk = disk_path.to_string_lossy().into_owned();
    plan.push(cmd(
        "Create the virtual hard disk (grows as Windows uses it)",
        &[
            "createmedium",
            "disk",
            "--filename",
            &disk,
            "--size",
            &size_mb,
            "--format",
            "VDI",
            "--variant",
            "Standard",
        ],
        600,
    ));

    let ctl = p.storage_controller_name.as_str();
    if !existing_controllers.iter().any(|c| c == ctl) {
        plan.push(cmd(
            "Add the storage controller",
            &[
                "storagectl",
                uuid,
                "--name",
                ctl,
                "--add",
                &p.storage_bus,
                "--controller",
                &p.storage_controller_type,
                "--portcount",
                "3",
                "--bootable",
                "on",
            ],
            120,
        ));
    } else {
        plan.push(cmd(
            "Make room on the storage controller",
            &["storagectl", uuid, "--name", ctl, "--portcount", "3"],
            120,
        ));
    }
    plan.push(cmd(
        "Attach the virtual hard disk",
        &[
            "storageattach",
            uuid,
            "--storagectl",
            ctl,
            "--port",
            "0",
            "--device",
            "0",
            "--type",
            "hdd",
            "--medium",
            &disk,
        ],
        120,
    ));
    let iso = spec.install_iso.to_string_lossy().into_owned();
    plan.push(cmd(
        "Insert the Windows installation disc",
        &[
            "storageattach",
            uuid,
            "--storagectl",
            ctl,
            "--port",
            "1",
            "--device",
            "0",
            "--type",
            "dvddrive",
            "--medium",
            &iso,
        ],
        120,
    ));
    if let Some(ga) = &spec.guest_additions_iso {
        let ga = ga.to_string_lossy().into_owned();
        plan.push(cmd(
            "Insert the VirtualBox Guest Additions disc",
            &[
                "storageattach",
                uuid,
                "--storagectl",
                ctl,
                "--port",
                "2",
                "--device",
                "0",
                "--type",
                "dvddrive",
                "--medium",
                &ga,
            ],
            120,
        ));
    }

    if p.secure_boot {
        plan.push(cmd(
            "Prepare UEFI Secure Boot keys (required by Windows 11)",
            &["modifynvram", uuid, "inituefivarstore"],
            120,
        ));
        plan.push(cmd(
            "Enroll Microsoft Secure Boot signatures",
            &["modifynvram", uuid, "enrollmssignatures"],
            120,
        ));
        plan.push(cmd(
            "Enroll the platform key",
            &["modifynvram", uuid, "enrollorclpk"],
            120,
        ));
        plan.push(cmd(
            "Turn on Secure Boot",
            &["modifynvram", uuid, "secureboot", "--enable"],
            120,
        ));
    }

    plan.push(cmd(
        "Record that this app created the VM",
        &[
            "setextradata",
            uuid,
            super::EXTRADATA_CREATED_BY,
            &spec.app_version,
        ],
        60,
    ));
    plan.push(cmd(
        "Record the setup id",
        &[
            "setextradata",
            uuid,
            super::EXTRADATA_INSTANCE_ID,
            &spec.instance_id,
        ],
        60,
    ));
    plan
}

pub fn disk_path_for(spec: &CreateVmSpec) -> PathBuf {
    // VirtualBox creates <basefolder>/<name>/ for the settings file; put the disk beside it.
    spec.base_folder
        .join(&spec.name)
        .join(format!("{}.vdi", spec.name))
}

pub fn start(uuid: &str) -> VboxCommand {
    cmd("Start Windows", &["startvm", uuid, "--type", "gui"], 120)
}
pub fn acpi_shutdown(uuid: &str) -> VboxCommand {
    cmd(
        "Ask Windows to shut down",
        &["controlvm", uuid, "acpipowerbutton"],
        60,
    )
}
pub fn save_state(uuid: &str) -> VboxCommand {
    cmd("Save the VM state", &["controlvm", uuid, "savestate"], 600)
}
pub fn power_off(uuid: &str) -> VboxCommand {
    cmd("Force power off", &["controlvm", uuid, "poweroff"], 120)
}
pub fn showvminfo(uuid: &str) -> VboxCommand {
    cmd(
        "Read VM settings",
        &["showvminfo", uuid, "--machinereadable"],
        60,
    )
}
pub fn guest_properties(uuid: &str) -> VboxCommand {
    cmd(
        "Read guest status",
        &["guestproperty", "enumerate", uuid],
        60,
    )
}
pub fn get_extradata(uuid: &str, key: &str) -> VboxCommand {
    cmd("Read VM marker", &["getextradata", uuid, key], 60)
}
pub fn eject_dvd(uuid: &str, controller: &str, port: u32, running: bool) -> VboxCommand {
    let p = port.to_string();
    let mut args = vec![
        "storageattach",
        uuid,
        "--storagectl",
        controller,
        "--port",
        &p,
        "--device",
        "0",
        "--type",
        "dvddrive",
        "--medium",
        "emptydrive",
    ];
    if running {
        args.push("--forceunmount");
    }
    cmd("Eject the disc", &args, 60)
}
pub fn attach_dvd(uuid: &str, controller: &str, port: u32, iso: &Path) -> VboxCommand {
    let p = port.to_string();
    let iso = iso.to_string_lossy().into_owned();
    cmd(
        "Insert disc",
        &[
            "storageattach",
            uuid,
            "--storagectl",
            controller,
            "--port",
            &p,
            "--device",
            "0",
            "--type",
            "dvddrive",
            "--medium",
            &iso,
        ],
        60,
    )
}
pub fn set_nic_type(uuid: &str, nic_type: &str) -> VboxCommand {
    cmd(
        "Change the virtual network adapter",
        &["modifyvm", uuid, "--nic-type1", nic_type],
        60,
    )
}
pub fn set_cable(uuid: &str, connected: bool, running: bool) -> VboxCommand {
    if running {
        cmd(
            "Toggle the network cable",
            &[
                "controlvm",
                uuid,
                "setlinkstate1",
                if connected { "on" } else { "off" },
            ],
            60,
        )
    } else {
        cmd(
            "Toggle the network cable",
            &[
                "modifyvm",
                uuid,
                "--cable-connected1",
                if connected { "on" } else { "off" },
            ],
            60,
        )
    }
}
pub fn unregister_and_delete(uuid: &str) -> VboxCommand {
    cmd(
        "Delete the VM and its files",
        &["unregistervm", uuid, "--delete-all"],
        600,
    )
}
pub fn unregister_keep_files(uuid: &str) -> VboxCommand {
    cmd(
        "Unregister the VM (keep files)",
        &["unregistervm", uuid],
        120,
    )
}

/// Names VirtualBox accepts; we additionally forbid path separators and control characters so
/// the VM folder is predictable.
pub fn validate_vm_name(name: &str) -> std::result::Result<(), String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("Please give the VM a name.".into());
    }
    if n.chars().count() > 64 {
        return Err("The VM name is too long (64 characters maximum).".into());
    }
    if n.chars()
        .any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
    {
        return Err("The VM name cannot contain / \\ : * ? \" < > | or control characters.".into());
    }
    if n != name {
        return Err("The VM name cannot start or end with spaces.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::profile_for;

    fn spec(arch: GuestArch, name: &str) -> CreateVmSpec {
        CreateVmSpec {
            name: name.into(),
            base_folder: PathBuf::from("/Users/Émilie Dupont/VirtualBox VMs"),
            sizing: VmSizing {
                ram_mb: 8192,
                cpus: 4,
                disk_gb: 100,
            },
            install_iso: PathBuf::from("/Users/Émilie Dupont/Downloads/Win11 (1).iso"),
            guest_additions_iso: Some(PathBuf::from(
                "/Applications/VirtualBox.app/Contents/MacOS/VBoxGuestAdditions.iso",
            )),
            profile: profile_for(arch),
            instance_id: "inst-123".into(),
            app_version: "0.1.0".into(),
        }
    }

    #[test]
    fn createvm_passes_names_verbatim() {
        let s = spec(GuestArch::Arm64, "Émilie's Windows {test}");
        let c = createvm(&s);
        assert_eq!(c.args[0], "createvm");
        assert!(c.args.contains(&"Émilie's Windows {test}".to_string()));
        assert!(c
            .args
            .windows(2)
            .any(|w| w[0] == "--platform-architecture" && w[1] == "arm"));
        assert!(c
            .args
            .windows(2)
            .any(|w| w[0] == "--ostype" && w[1] == "Windows11_arm64"));
        assert!(c.args.contains(&"--default".to_string()));
        assert!(c.args.contains(&"--register".to_string()));
        // No argument is ever a composed shell string.
        assert!(c
            .args
            .iter()
            .all(|a| !a.contains("&&") && !a.contains(" --")));
    }

    #[test]
    fn arm_plan_has_no_x86_only_settings() {
        let s = spec(GuestArch::Arm64, "Windows 11");
        let plan = configure(&s, "uuid-1", &["SATA".to_string()], &disk_path_for(&s));
        let modify = &plan[0].args;
        assert!(!modify.contains(&"--tpm-type".to_string()));
        assert!(!modify.contains(&"--firmware".to_string()));
        assert!(!modify.contains(&"--chipset".to_string()));
        assert!(!modify.contains(&"--boot1".to_string()));
        assert!(modify
            .windows(2)
            .any(|w| w[0] == "--nic-type1" && w[1] == "usbnet"));
        assert!(modify
            .windows(2)
            .any(|w| w[0] == "--graphicscontroller" && w[1] == "vboxsvga"));
        // Existing SATA controller is reused, not re-added.
        assert!(!plan.iter().any(|c| c.args.contains(&"--add".to_string())));
        assert!(plan
            .iter()
            .any(|c| c.args[0] == "modifynvram"
                && c.args.contains(&"enrollmssignatures".to_string())));
    }

    #[test]
    fn x64_plan_sets_efi_tpm_and_adds_controller() {
        let s = spec(GuestArch::X64, "Windows 11");
        let plan = configure(&s, "uuid-2", &[], &disk_path_for(&s));
        let modify = &plan[0].args;
        assert!(modify
            .windows(2)
            .any(|w| w[0] == "--tpm-type" && w[1] == "2.0"));
        assert!(modify
            .windows(2)
            .any(|w| w[0] == "--firmware" && w[1] == "efi"));
        assert!(modify
            .windows(2)
            .any(|w| w[0] == "--nic-type1" && w[1] == "82540EM"));
        let add = plan.iter().find(|c| c.args[0] == "storagectl").unwrap();
        assert!(add.args.contains(&"--add".to_string()));
        let disk = plan.iter().find(|c| c.args[0] == "createmedium").unwrap();
        assert!(disk
            .args
            .windows(2)
            .any(|w| w[0] == "--size" && w[1] == "102400"));
        assert!(disk
            .args
            .windows(2)
            .any(|w| w[0] == "--variant" && w[1] == "Standard"));
        let iso = plan
            .iter()
            .find(|c| c.args.contains(&"dvddrive".to_string()))
            .unwrap();
        assert!(iso
            .args
            .contains(&"/Users/Émilie Dupont/Downloads/Win11 (1).iso".to_string()));
        assert_eq!(
            disk_path_for(&s),
            PathBuf::from("/Users/Émilie Dupont/VirtualBox VMs/Windows 11/Windows 11.vdi")
        );
        assert!(plan
            .iter()
            .any(|c| c.args[0] == "setextradata" && c.args.contains(&"inst-123".to_string())));
    }

    #[test]
    fn vm_name_validation() {
        assert!(validate_vm_name("Windows 11").is_ok());
        assert!(validate_vm_name("Émilie's PC").is_ok());
        assert!(validate_vm_name("").is_err());
        assert!(validate_vm_name("a/b").is_err());
        assert!(validate_vm_name(" x").is_err());
        assert!(validate_vm_name(&"x".repeat(65)).is_err());
    }

    #[test]
    fn new_vms_do_not_share_clipboard_or_drag_and_drop() {
        for arch in [GuestArch::Arm64, GuestArch::X64] {
            let s = spec(arch, "Desktop");
            let steps = configure(&s, "u", &[], &disk_path_for(&s));
            let privacy = steps
                .iter()
                .find(|s| s.description == "Disable host clipboard and drag-and-drop sharing")
                .unwrap();
            assert_eq!(
                privacy.args,
                vec![
                    "modifyvm",
                    "u",
                    "--clipboard-mode",
                    "disabled",
                    "--clipboard-file-transfers",
                    "disabled",
                    "--drag-and-drop",
                    "disabled"
                ]
            );
        }
    }

    #[test]
    fn eject_when_running_forces_unmount() {
        assert!(eject_dvd("u", "SATA", 1, true)
            .args
            .contains(&"--forceunmount".to_string()));
        assert!(!eject_dvd("u", "SATA", 1, false)
            .args
            .contains(&"--forceunmount".to_string()));
        assert_eq!(set_cable("u", false, true).args[2], "setlinkstate1");
    }
}
