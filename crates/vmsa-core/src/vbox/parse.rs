//! Parsers for VBoxManage output. Tested against recorded, sanitized output in
//! `tests/fixtures/vboxmanage/`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{CoreError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VmListEntry {
    pub name: String,
    pub uuid: String,
}

/// `VBoxManage list vms` / `list runningvms`: one `"name" {uuid}` per line.
pub fn parse_vm_list(text: &str) -> Vec<VmListEntry> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            let end_name = line.rfind("\" {")?;
            let name = line.get(1..end_name)?;
            let uuid = line[end_name + 3..].trim_end_matches('}');
            if !line.starts_with('"') || uuid.len() != 36 {
                return None;
            }
            Some(VmListEntry {
                name: name.to_string(),
                uuid: uuid.to_string(),
            })
        })
        .collect()
}

/// `VBoxManage showvminfo --machinereadable`: `key="value"` or `key=value` lines.
/// Keys can themselves be quoted (`"SATA-0-0"="/path"`).
pub fn parse_machine_readable(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        let Some((k, v)) = split_kv(line) else {
            continue;
        };
        map.insert(k, v);
    }
    map
}

fn split_kv(line: &str) -> Option<(String, String)> {
    let (key, rest) = if let Some(stripped) = line.strip_prefix('"') {
        let end = stripped.find('"')?;
        let key = &stripped[..end];
        let rest = stripped[end + 1..].strip_prefix('=')?;
        (key, rest)
    } else {
        let i = line.find('=')?;
        (&line[..i], &line[i + 1..])
    };
    let value = rest
        .strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .unwrap_or(rest);
    Some((key.to_string(), value.to_string()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VmState {
    PoweredOff,
    Saved,
    Running,
    Paused,
    Aborted,
    Starting,
    Stopping,
    Saving,
    Restoring,
    Other,
}

impl VmState {
    pub fn parse(s: &str) -> VmState {
        match s {
            "poweroff" => VmState::PoweredOff,
            "saved" => VmState::Saved,
            "running" => VmState::Running,
            "paused" => VmState::Paused,
            "aborted" | "aborted-saved" => VmState::Aborted,
            "starting" => VmState::Starting,
            "stopping" => VmState::Stopping,
            "saving" => VmState::Saving,
            "restoring" => VmState::Restoring,
            _ => VmState::Other,
        }
    }
    pub fn is_live(&self) -> bool {
        matches!(
            self,
            VmState::Running
                | VmState::Paused
                | VmState::Starting
                | VmState::Stopping
                | VmState::Saving
                | VmState::Restoring
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageAttachment {
    pub controller: String,
    pub port: u32,
    pub device: u32,
    pub medium: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VmInfo {
    pub name: String,
    pub uuid: String,
    pub state: VmState,
    pub state_raw: String,
    pub state_change_time: Option<String>,
    pub ostype: String,
    pub platform_architecture: Option<String>,
    pub memory_mb: Option<u32>,
    pub cpus: Option<u32>,
    pub vram_mb: Option<u32>,
    pub firmware: Option<String>,
    pub graphics_controller: Option<String>,
    pub chipset: Option<String>,
    pub tpm_type: Option<String>,
    pub config_file: Option<String>,
    pub log_folder: Option<String>,
    pub nic1: Option<String>,
    pub nic1_type: Option<String>,
    pub nic1_cable_connected: Option<bool>,
    pub storage_controllers: Vec<String>,
    pub attachments: Vec<StorageAttachment>,
    pub guest_additions_version: Option<String>,
    pub guest_additions_run_level: Option<u32>,
    pub raw: BTreeMap<String, String>,
}

pub fn parse_vm_info(text: &str) -> Result<VmInfo> {
    let m = parse_machine_readable(text);
    let name = m
        .get("name")
        .cloned()
        .ok_or_else(|| CoreError::Parse("showvminfo output has no name".into()))?;
    let uuid = m
        .get("UUID")
        .cloned()
        .ok_or_else(|| CoreError::Parse("showvminfo output has no UUID".into()))?;
    let state_raw = m.get("VMState").cloned().unwrap_or_default();
    let get = |k: &str| m.get(k).cloned();
    let num = |k: &str| m.get(k).and_then(|v| v.parse::<u32>().ok());

    let mut storage_controllers = Vec::new();
    for i in 0..16 {
        if let Some(n) = m.get(&format!("storagecontrollername{i}")) {
            storage_controllers.push(n.clone());
        }
    }
    let mut attachments = Vec::new();
    for c in &storage_controllers {
        for port in 0..30u32 {
            for dev in 0..2u32 {
                if let Some(v) = m.get(&format!("{c}-{port}-{dev}")) {
                    attachments.push(StorageAttachment {
                        controller: c.clone(),
                        port,
                        device: dev,
                        medium: v.clone(),
                    });
                }
            }
        }
    }
    Ok(VmInfo {
        name,
        uuid,
        state: VmState::parse(&state_raw),
        state_raw,
        state_change_time: get("VMStateChangeTime"),
        ostype: get("ostype").unwrap_or_default(),
        platform_architecture: get("platformArchitecture"),
        memory_mb: num("memory"),
        cpus: num("cpus"),
        vram_mb: num("vram"),
        firmware: get("firmware"),
        graphics_controller: get("graphicscontroller"),
        chipset: get("chipset"),
        tpm_type: get("tpmtype"),
        config_file: get("CfgFile"),
        log_folder: get("LogFldr"),
        nic1: get("nic1"),
        nic1_type: get("nictype1"),
        nic1_cable_connected: get("cableconnected1").map(|v| v == "on"),
        guest_additions_version: get("GuestAdditionsVersion"),
        guest_additions_run_level: num("GuestAdditionsRunLevel"),
        storage_controllers,
        attachments,
        raw: m,
    })
}

/// `VBoxManage --version` prints e.g. `7.2.20r175154`.
pub fn parse_version(text: &str) -> Option<crate::profile::VboxVersion> {
    // Warnings (e.g. about kernel modules) may precede the version; take the last plausible line.
    text.lines()
        .rev()
        .map(str::trim)
        .find_map(crate::profile::VboxVersion::parse)
}

/// `VBoxManage createvm` prints `UUID: <uuid>` and `Settings file: '<path>'`.
pub fn parse_createvm(text: &str) -> Option<(String, Option<String>)> {
    let mut uuid = None;
    let mut settings = None;
    for line in text.lines() {
        if let Some(v) = line.trim().strip_prefix("UUID:") {
            uuid = Some(v.trim().to_string());
        } else if let Some(v) = line.trim().strip_prefix("Settings file:") {
            settings = Some(v.trim().trim_matches('\'').to_string());
        }
    }
    uuid.map(|u| (u, settings))
}

/// `VBoxManage guestproperty enumerate`: `Name: X, value: Y, timestamp: N, flags: F`.
pub fn parse_guest_properties(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("Name:") else {
            continue;
        };
        let Some((name, rest)) = rest.split_once(", value:") else {
            continue;
        };
        let value = match rest.find(", timestamp:") {
            Some(i) => &rest[..i],
            None => rest.split(", flags:").next().unwrap_or(rest),
        };
        map.insert(name.trim().to_string(), value.trim().to_string());
    }
    map
}

/// `VBoxManage guestproperty get VM key` prints `Value: X` or `No value set!`.
pub fn parse_guest_property_get(text: &str) -> Option<String> {
    text.lines().find_map(|l| {
        l.trim()
            .strip_prefix("Value:")
            .map(|v| v.trim().to_string())
    })
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostInfoOutput {
    pub processor_online_count: Option<u32>,
    pub memory_size_mb: Option<u64>,
    pub memory_available_mb: Option<u64>,
    pub supports_hw_virt: Option<bool>,
    pub os: Option<String>,
    pub os_version: Option<String>,
}

/// `VBoxManage list hostinfo`.
pub fn parse_hostinfo(text: &str) -> HostInfoOutput {
    let mut out = HostInfoOutput::default();
    for line in text.lines() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let k = k.trim();
        let v = v.trim();
        let first_num = || {
            v.split_whitespace()
                .next()
                .and_then(|n| n.parse::<u64>().ok())
        };
        match k {
            "Processor online count" => out.processor_online_count = first_num().map(|n| n as u32),
            "Memory size" => out.memory_size_mb = first_num(),
            "Memory available" => out.memory_available_mb = first_num(),
            "Processor supports HW virtualization" => {
                out.supports_hw_virt = Some(v.eq_ignore_ascii_case("yes"))
            }
            "Operating system" => out.os = Some(v.to_string()),
            "Operating system version" => out.os_version = Some(v.to_string()),
            _ => {}
        }
    }
    out
}

/// `VBoxManage list systemproperties` → default machine folder.
pub fn parse_default_machine_folder(text: &str) -> Option<String> {
    text.lines().find_map(|l| {
        l.strip_prefix("Default machine folder:")
            .map(|v| v.trim().to_string())
    })
}

/// `VBoxManage getextradata VM key` prints `Value: X` or `No value set!`.
pub fn parse_extradata_get(text: &str) -> Option<String> {
    parse_guest_property_get(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_DIR: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/vboxmanage"
    );

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{FIXTURE_DIR}/{name}"))
            .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
    }

    #[test]
    fn list_vms() {
        let v = parse_vm_list(&fixture("list_vms.txt"));
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].name, "Windows 11");
        assert_eq!(v[1].name, "Ünïcödé VM with spaces {and braces}");
        assert_eq!(v[2].name, "<inaccessible>");
        assert_eq!(v[0].uuid.len(), 36);
        assert!(parse_vm_list("").is_empty());
        assert!(parse_vm_list("WARNING: something\n").is_empty());
    }

    #[test]
    fn showvminfo_arm64() {
        let info = parse_vm_info(&fixture("showvminfo_win11_arm64_poweroff.txt")).unwrap();
        assert_eq!(info.name, "Windows 11");
        assert_eq!(info.state, VmState::PoweredOff);
        assert_eq!(info.ostype, "Windows11_arm64");
        assert_eq!(info.platform_architecture.as_deref(), Some("ARM"));
        assert_eq!(info.memory_mb, Some(8192));
        assert_eq!(info.cpus, Some(4));
        assert_eq!(info.nic1.as_deref(), Some("nat"));
        assert_eq!(info.nic1_type.as_deref(), Some("usbnet"));
        assert_eq!(info.nic1_cable_connected, Some(true));
        assert_eq!(info.storage_controllers, vec!["SATA".to_string()]);
        assert_eq!(info.attachments.len(), 3);
        assert!(info.attachments[0].medium.ends_with("Windows 11.vdi"));
        assert_eq!(info.graphics_controller.as_deref(), Some("VBoxSVGA"));
        assert_eq!(info.chipset.as_deref(), Some("armv8virtual"));
        assert!(info.log_folder.as_deref().unwrap().contains("Logs"));
    }

    #[test]
    fn showvminfo_x64_running_with_additions() {
        let info = parse_vm_info(&fixture("showvminfo_win11_x64_running.txt")).unwrap();
        assert_eq!(info.state, VmState::Running);
        assert!(info.state.is_live());
        assert_eq!(info.firmware.as_deref(), Some("EFI"));
        assert_eq!(info.tpm_type.as_deref(), Some("2.0"));
        assert_eq!(info.nic1_type.as_deref(), Some("82540EM"));
        assert_eq!(
            info.guest_additions_version.as_deref(),
            Some("7.2.20 r175154")
        );
        assert_eq!(info.guest_additions_run_level, Some(3));
    }

    #[test]
    fn version_lines() {
        assert_eq!(
            parse_version("7.2.20r175154\n").unwrap().to_string(),
            "7.2.20r175154"
        );
        assert_eq!(
            parse_version("WARNING: The vboxdrv kernel module is not loaded.\n7.0.14r161095\n")
                .unwrap()
                .patch,
            14
        );
        assert!(parse_version("").is_none());
    }

    #[test]
    fn createvm_output() {
        let (uuid, settings) = parse_createvm(&fixture("createvm.txt")).unwrap();
        assert_eq!(uuid, "3f2504e0-4f89-41d3-9a0c-0305e82c3301");
        assert!(settings.unwrap().ends_with("Windows 11.vbox"));
    }

    #[test]
    fn guest_properties() {
        let m = parse_guest_properties(&fixture("guestproperty_enumerate.txt"));
        assert_eq!(
            m.get("/VirtualBox/GuestInfo/Net/0/V4/IP")
                .map(String::as_str),
            Some("10.0.2.15")
        );
        assert_eq!(
            m.get("/VirtualBox/GuestInfo/Net/0/Status")
                .map(String::as_str),
            Some("Up")
        );
        assert_eq!(
            m.get("/VirtualBox/GuestAdd/Version").map(String::as_str),
            Some("7.2.20")
        );
        assert_eq!(
            parse_guest_property_get("Value: 7.2.20\n").as_deref(),
            Some("7.2.20")
        );
        assert!(parse_guest_property_get("No value set!\n").is_none());
    }

    #[test]
    fn hostinfo() {
        let h = parse_hostinfo(&fixture("list_hostinfo_mac_arm.txt"));
        assert_eq!(h.processor_online_count, Some(12));
        assert_eq!(h.memory_size_mb, Some(24576));
        assert_eq!(h.supports_hw_virt, Some(true));
        assert_eq!(h.os.as_deref(), Some("Darwin"));
    }

    #[test]
    fn systemproperties() {
        let f = parse_default_machine_folder(&fixture("list_systemproperties.txt")).unwrap();
        assert!(f.ends_with("VirtualBox VMs"));
    }

    #[test]
    fn machine_readable_edge_cases() {
        let m = parse_machine_readable(
            "name=\"a=b\"\n\"SATA-0-0\"=\"C:\\x y\\d.vdi\"\nbare=value\n\nnovalue\n",
        );
        assert_eq!(m["name"], "a=b");
        assert_eq!(m["SATA-0-0"], "C:\\x y\\d.vdi");
        assert_eq!(m["bare"], "value");
        assert!(!m.contains_key("novalue"));
    }
}
