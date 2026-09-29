//! Guest-visible VM identity configuration for compatibility testing.
//!
//! Some Windows software, drivers and installers behave differently, or refuse to run, when they
//! detect a virtual machine. This module lets a tester change the identifiers a guest can read so a
//! VM can be exercised as if it were a specific physical machine. It uses only Oracle's documented,
//! first-party `VBoxManage` mechanisms:
//!
//! - DMI/SMBIOS strings via `VBoxInternal/Devices/pcbios/0/Config/Dmi*`
//!   (VirtualBox manual, "Configuring the BIOS DMI Information"),
//! - ATA identity strings via `VBoxInternal/Devices/<controller>/0/Config/Port<n>/{SerialNumber,
//!   ModelNumber,FirmwareRevision}`,
//! - network adapter identity and mode via `modifyvm --macaddress1 / --nic-type1 / --nic1`,
//! - the SMBIOS system UUID via `modifyvm --hardwareuuid`,
//! - the paravirtualization interface (hypervisor CPUID leaf) via `modifyvm --paravirtprovider`.
//!
//! Everything here is pure: it builds argument arrays and never runs a process, which keeps the
//! exact commands testable. The whole feature is **off by default** (`IdentityConfig::enabled` is
//! `false`); a disabled config produces no commands and leaves the profile defaults untouched.
//!
//! This does not, and cannot, make a VM undetectable. `remaining_indicators()` documents the
//! signals that stay visible regardless of these settings; see also
//! `docs/VM-IDENTITY.md` and `docs/IDENTITY-CHECKLIST.md`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::profile::VmProfile;
use crate::vbox::plan::VboxCommand;

/// Base extra-data path for the emulated BIOS/SMBIOS device.
const PCBIOS_CFG: &str = "VBoxInternal/Devices/pcbios/0/Config";
/// VirtualBox-specific SMBIOS OEM strings (DMI type 11). VirtualBox fills these with
/// `vboxVer_<version>` / `vboxRev_<revision>` by default, which is a well-known VM tell.
const OEM_VBOX_VER: &str = "DmiOEMVBoxVer";
const OEM_VBOX_REV: &str = "DmiOEMVBoxRev";

/// Practical SMBIOS string limit; VirtualBox stores these as extra data with no fixed cap, but
/// real firmware keeps DMI strings short, so we reject implausibly long values.
const DMI_MAX: usize = 64;
/// ATA/ATAPI IDENTIFY field widths (in characters).
const ATA_SERIAL_MAX: usize = 20;
const ATA_MODEL_MAX: usize = 40;
const ATA_FIRMWARE_MAX: usize = 8;

const PARAVIRT_PROVIDERS: &[&str] = &["default", "none", "legacy", "minimal", "hyperv", "kvm"];
/// Adapter models VirtualBox emulates and that this app supports selecting.
pub const ADAPTER_MODELS: &[&str] = &[
    "82540EM", "82543GC", "82545EM", "Am79C973", "virtio", "usbnet",
];

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Firmware (BIOS) identity as reported through SMBIOS type 0.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirmwareIdentity {
    #[serde(default)]
    pub bios_vendor: Option<String>,
    #[serde(default)]
    pub bios_version: Option<String>,
    /// Free-form release date string, e.g. `12/31/2023` (SMBIOS convention).
    #[serde(default)]
    pub bios_release_date: Option<String>,
}

/// System, mainboard and chassis identity (SMBIOS types 1, 2 and 3).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemIdentity {
    #[serde(default)]
    pub manufacturer: Option<String>,
    #[serde(default)]
    pub product_name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub serial_number: Option<String>,
    #[serde(default)]
    pub sku: Option<String>,
    #[serde(default)]
    pub family: Option<String>,
    /// SMBIOS system UUID. Also applied to `modifyvm --hardwareuuid` so both agree.
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub board_manufacturer: Option<String>,
    #[serde(default)]
    pub board_product: Option<String>,
    #[serde(default)]
    pub board_serial: Option<String>,
    #[serde(default)]
    pub chassis_manufacturer: Option<String>,
    #[serde(default)]
    pub chassis_asset_tag: Option<String>,
}

/// Virtual disk identity as reported through the ATA IDENTIFY DEVICE response.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageIdentity {
    #[serde(default)]
    pub disk_serial: Option<String>,
    #[serde(default)]
    pub disk_model: Option<String>,
    #[serde(default)]
    pub disk_firmware_revision: Option<String>,
}

/// How the guest's first network adapter is presented and attached.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum NetworkMode {
    /// VirtualBox's built-in NAT (the profile default). Full outbound connectivity, guest is not
    /// reachable from the LAN.
    #[default]
    Nat,
    /// A named internal NAT network shared between VMs.
    NatNetwork { name: String },
    /// Bridged onto a named host adapter; the guest appears as a peer on the physical LAN.
    Bridged { host_adapter: String },
    /// Host-only network on a named adapter; guest reaches the host only, no outbound internet.
    HostOnly { host_adapter: String },
}

impl NetworkMode {
    pub fn label(&self) -> &'static str {
        match self {
            NetworkMode::Nat => "NAT (default)",
            NetworkMode::NatNetwork { .. } => "NAT network",
            NetworkMode::Bridged { .. } => "Bridged",
            NetworkMode::HostOnly { .. } => "Host-only",
        }
    }
    /// Plain-language description of what this mode means for connectivity.
    pub fn connectivity_note(&self) -> &'static str {
        match self {
            NetworkMode::Nat =>
                "Outbound internet works; the guest is hidden behind the host and not reachable from the LAN. The guest sees a 10.0.2.x address.",
            NetworkMode::NatNetwork { .. } =>
                "Outbound internet works and VMs on the same named network can reach each other; still not reachable from the physical LAN.",
            NetworkMode::Bridged { .. } =>
                "The guest gets an address from the physical LAN's DHCP and is reachable like a real machine. Needs a working host adapter; some corporate or Wi-Fi networks block bridging.",
            NetworkMode::HostOnly { .. } =>
                "The guest can reach only the host, with no outbound internet. Use this to isolate a test VM.",
        }
    }
}

/// Levers that reduce guest-visible VirtualBox branding without removing drivers or management.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrandingReduction {
    /// Paravirtualization interface. `none` removes the hypervisor CPUID leaf (bit 31 of CPUID.1
    /// ECX and the `VBoxVBoxVBox` signature at leaf 0x40000000), the most direct "I am a VM" hint,
    /// at the cost of losing paravirtual time sync and some performance. `default` keeps the
    /// profile behaviour.
    #[serde(default)]
    pub paravirt_provider: Option<String>,
    /// Overwrite the VirtualBox-specific SMBIOS OEM strings (`vboxVer_*` / `vboxRev_*`, DMI type 11)
    /// with a neutral value so they no longer name VirtualBox.
    #[serde(default)]
    pub clear_vbox_oem_strings: bool,
}

/// The complete guest-visible identity configuration. Off unless `enabled` is set.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityConfig {
    /// Master switch. When `false` (the default), no identity commands are generated and the VM
    /// keeps the profile defaults.
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub firmware: FirmwareIdentity,
    #[serde(default)]
    pub system: SystemIdentity,
    #[serde(default)]
    pub storage: StorageIdentity,
    #[serde(default)]
    pub mac_address: Option<String>,
    #[serde(default)]
    pub adapter_model: Option<String>,
    #[serde(default)]
    pub network_mode: NetworkMode,
    #[serde(default)]
    pub branding: BrandingReduction,
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

fn check_dmi(problems: &mut Vec<String>, label: &str, v: &Option<String>) {
    if let Some(s) = v {
        if s.chars().any(|c| c.is_control()) {
            problems.push(format!("{label} cannot contain control characters."));
        }
        if s.chars().count() > DMI_MAX {
            problems.push(format!(
                "{label} is too long ({} characters); keep it to {DMI_MAX} or fewer.",
                s.chars().count()
            ));
        }
    }
}

fn check_ata(problems: &mut Vec<String>, label: &str, v: &Option<String>, max: usize) {
    if let Some(s) = v {
        if s.chars().any(|c| c.is_control()) {
            problems.push(format!("{label} cannot contain control characters."));
        }
        if s.chars().count() > max {
            problems.push(format!(
                "{label} is too long ({} characters); the drive field holds at most {max}.",
                s.chars().count()
            ));
        }
    }
}

/// Normalize a MAC to 12 uppercase hex digits, accepting `:` / `-` separators. Returns `None`
/// when it is not 12 hex digits.
pub fn normalize_mac(mac: &str) -> Option<String> {
    let cleaned: String = mac
        .chars()
        .filter(|c| *c != ':' && *c != '-' && *c != '.')
        .collect();
    if cleaned.len() == 12 && cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(cleaned.to_ascii_uppercase())
    } else {
        None
    }
}

/// Validate a configuration. Returns human-readable problems; an empty vector means it is safe to
/// apply. A disabled config is always valid (it changes nothing).
pub fn validate(cfg: &IdentityConfig) -> Vec<String> {
    let mut problems = Vec::new();
    if !cfg.enabled {
        return problems;
    }

    check_dmi(&mut problems, "BIOS vendor", &cfg.firmware.bios_vendor);
    check_dmi(&mut problems, "BIOS version", &cfg.firmware.bios_version);
    check_dmi(
        &mut problems,
        "BIOS release date",
        &cfg.firmware.bios_release_date,
    );

    check_dmi(
        &mut problems,
        "System manufacturer",
        &cfg.system.manufacturer,
    );
    check_dmi(
        &mut problems,
        "System product name",
        &cfg.system.product_name,
    );
    check_dmi(&mut problems, "System version", &cfg.system.version);
    check_dmi(
        &mut problems,
        "System serial number",
        &cfg.system.serial_number,
    );
    check_dmi(&mut problems, "System SKU", &cfg.system.sku);
    check_dmi(&mut problems, "System family", &cfg.system.family);
    check_dmi(
        &mut problems,
        "Board manufacturer",
        &cfg.system.board_manufacturer,
    );
    check_dmi(&mut problems, "Board product", &cfg.system.board_product);
    check_dmi(
        &mut problems,
        "Board serial number",
        &cfg.system.board_serial,
    );
    check_dmi(
        &mut problems,
        "Chassis manufacturer",
        &cfg.system.chassis_manufacturer,
    );
    check_dmi(
        &mut problems,
        "Chassis asset tag",
        &cfg.system.chassis_asset_tag,
    );

    if let Some(uuid) = &cfg.system.uuid {
        if uuid::Uuid::parse_str(uuid).is_err() {
            problems.push(
                "System UUID must be a valid UUID, e.g. 12345678-1234-1234-1234-1234567890ab."
                    .into(),
            );
        }
    }

    check_ata(
        &mut problems,
        "Disk serial number",
        &cfg.storage.disk_serial,
        ATA_SERIAL_MAX,
    );
    check_ata(
        &mut problems,
        "Disk model",
        &cfg.storage.disk_model,
        ATA_MODEL_MAX,
    );
    check_ata(
        &mut problems,
        "Disk firmware revision",
        &cfg.storage.disk_firmware_revision,
        ATA_FIRMWARE_MAX,
    );

    if let Some(mac) = &cfg.mac_address {
        match normalize_mac(mac) {
            None => problems
                .push("MAC address must be 12 hexadecimal digits, e.g. 080027AABBCC.".into()),
            Some(norm) => {
                // First octet: bit 0 is the multicast bit; a unicast NIC address must be even.
                let first = u8::from_str_radix(&norm[0..2], 16).unwrap_or(0);
                if first & 0x01 != 0 {
                    problems.push(
                        "MAC address is a multicast address (first octet is odd); use a unicast address.".into(),
                    );
                }
                if norm == "000000000000" {
                    problems.push("MAC address cannot be all zeros.".into());
                }
            }
        }
    }

    if let Some(model) = &cfg.adapter_model {
        if !ADAPTER_MODELS.contains(&model.as_str()) {
            problems.push(format!(
                "Unknown network adapter model '{model}'. Supported: {}.",
                ADAPTER_MODELS.join(", ")
            ));
        }
    }

    match &cfg.network_mode {
        NetworkMode::Nat => {}
        NetworkMode::NatNetwork { name } => {
            if name.trim().is_empty() {
                problems.push("A NAT network needs a name.".into());
            }
        }
        NetworkMode::Bridged { host_adapter } => {
            if host_adapter.trim().is_empty() {
                problems
                    .push("Bridged networking needs the name of a host network adapter.".into());
            }
        }
        NetworkMode::HostOnly { host_adapter } => {
            if host_adapter.trim().is_empty() {
                problems.push("Host-only networking needs the name of a host-only adapter.".into());
            }
        }
    }

    if let Some(p) = &cfg.branding.paravirt_provider {
        if !PARAVIRT_PROVIDERS.contains(&p.as_str()) {
            problems.push(format!(
                "Unknown paravirtualization provider '{p}'. Supported: {}.",
                PARAVIRT_PROVIDERS.join(", ")
            ));
        }
    }

    problems
}

// ---------------------------------------------------------------------------
// Command generation
// ---------------------------------------------------------------------------

fn vcmd(description: &str, args: Vec<String>) -> VboxCommand {
    VboxCommand {
        description: description.into(),
        args,
        timeout_secs: 60,
    }
}

fn set_extra(uuid: &str, key: &str, value: &str) -> VboxCommand {
    vcmd(
        "Set a guest-visible identifier",
        vec!["setextradata".into(), uuid.into(), key.into(), value.into()],
    )
}

/// Delete an extra-data key (VBoxManage removes a key when no value is given).
fn unset_extra(uuid: &str, key: &str) -> VboxCommand {
    vcmd(
        "Clear a guest-visible identifier",
        vec!["setextradata".into(), uuid.into(), key.into()],
    )
}

fn pcbios(key: &str) -> String {
    format!("{PCBIOS_CFG}/{key}")
}

fn storage_key(profile: &VmProfile, field: &str) -> String {
    // The disk is attached at port 0 (see vbox::plan::configure).
    format!(
        "VBoxInternal/Devices/{}/0/Config/Port0/{field}",
        profile.storage_device_key()
    )
}

/// All extra-data keys this feature may write, so callers can capture their current values before
/// applying (for a faithful revert) and clear them on revert.
pub fn managed_extradata_keys(profile: &VmProfile) -> Vec<String> {
    let mut keys: Vec<String> = [
        "DmiBIOSVendor",
        "DmiBIOSVersion",
        "DmiBIOSReleaseDate",
        "DmiSystemVendor",
        "DmiSystemProduct",
        "DmiSystemVersion",
        "DmiSystemSerial",
        "DmiSystemSKU",
        "DmiSystemFamily",
        "DmiSystemUuid",
        "DmiBoardVendor",
        "DmiBoardProduct",
        "DmiBoardSerial",
        "DmiChassisVendor",
        "DmiChassisAssetTag",
        OEM_VBOX_VER,
        OEM_VBOX_REV,
    ]
    .iter()
    .map(|k| pcbios(k))
    .collect();
    keys.push(storage_key(profile, "SerialNumber"));
    keys.push(storage_key(profile, "ModelNumber"));
    keys.push(storage_key(profile, "FirmwareRevision"));
    keys
}

/// `showvminfo --machinereadable` keys whose current values a caller should capture before
/// applying, so revert can restore the exact prior `modifyvm` state.
pub fn managed_modifyvm_keys() -> &'static [&'static str] {
    &[
        "nic1",
        "nictype1",
        "macaddress1",
        "hardwareuuid",
        "paravirtprovider",
    ]
}

/// Build the commands that apply a configuration to an existing VM (identified by `uuid`). Returns
/// an empty vector when the config is disabled or sets nothing. Assumes the VM is powered off.
pub fn apply_commands(cfg: &IdentityConfig, uuid: &str, profile: &VmProfile) -> Vec<VboxCommand> {
    let mut plan = Vec::new();
    if !cfg.enabled {
        return plan;
    }

    // --- SMBIOS / DMI strings ---
    let dmi: [(&str, &Option<String>); 15] = [
        ("DmiBIOSVendor", &cfg.firmware.bios_vendor),
        ("DmiBIOSVersion", &cfg.firmware.bios_version),
        ("DmiBIOSReleaseDate", &cfg.firmware.bios_release_date),
        ("DmiSystemVendor", &cfg.system.manufacturer),
        ("DmiSystemProduct", &cfg.system.product_name),
        ("DmiSystemVersion", &cfg.system.version),
        ("DmiSystemSerial", &cfg.system.serial_number),
        ("DmiSystemSKU", &cfg.system.sku),
        ("DmiSystemFamily", &cfg.system.family),
        ("DmiSystemUuid", &cfg.system.uuid),
        ("DmiBoardVendor", &cfg.system.board_manufacturer),
        ("DmiBoardProduct", &cfg.system.board_product),
        ("DmiBoardSerial", &cfg.system.board_serial),
        ("DmiChassisVendor", &cfg.system.chassis_manufacturer),
        ("DmiChassisAssetTag", &cfg.system.chassis_asset_tag),
    ];
    for (key, value) in dmi {
        if let Some(v) = value {
            plan.push(set_extra(uuid, &pcbios(key), v));
        }
    }

    if cfg.branding.clear_vbox_oem_strings {
        // A single space is a neutral, non-empty value so the key is set rather than deleted.
        plan.push(set_extra(uuid, &pcbios(OEM_VBOX_VER), " "));
        plan.push(set_extra(uuid, &pcbios(OEM_VBOX_REV), " "));
    }

    // --- Storage (ATA IDENTIFY) strings ---
    if let Some(v) = &cfg.storage.disk_serial {
        plan.push(set_extra(uuid, &storage_key(profile, "SerialNumber"), v));
    }
    if let Some(v) = &cfg.storage.disk_model {
        plan.push(set_extra(uuid, &storage_key(profile, "ModelNumber"), v));
    }
    if let Some(v) = &cfg.storage.disk_firmware_revision {
        plan.push(set_extra(
            uuid,
            &storage_key(profile, "FirmwareRevision"),
            v,
        ));
    }

    // --- One modifyvm for adapter identity, mode, hardware UUID and paravirt provider ---
    let mut modify: Vec<String> = vec!["modifyvm".into(), uuid.into()];
    match &cfg.network_mode {
        NetworkMode::Nat => modify.extend(["--nic1".into(), "nat".into()]),
        NetworkMode::NatNetwork { name } => {
            modify.extend([
                "--nic1".into(),
                "natnetwork".into(),
                "--nat-network1".into(),
                name.clone(),
            ]);
        }
        NetworkMode::Bridged { host_adapter } => {
            modify.extend([
                "--nic1".into(),
                "bridged".into(),
                "--bridge-adapter1".into(),
                host_adapter.clone(),
            ]);
        }
        NetworkMode::HostOnly { host_adapter } => {
            modify.extend([
                "--nic1".into(),
                "hostonly".into(),
                "--host-only-adapter1".into(),
                host_adapter.clone(),
            ]);
        }
    }
    if let Some(model) = &cfg.adapter_model {
        modify.extend(["--nic-type1".into(), model.clone()]);
    }
    if let Some(mac) = &cfg.mac_address.as_ref().and_then(|m| normalize_mac(m)) {
        modify.extend(["--macaddress1".into(), mac.clone()]);
    }
    if let Some(uuid_val) = &cfg.system.uuid {
        modify.extend(["--hardwareuuid".into(), uuid_val.clone()]);
    }
    if let Some(p) = &cfg.branding.paravirt_provider {
        modify.extend(["--paravirtprovider".into(), p.clone()]);
    }
    // Only push if we added at least one real flag beyond `modifyvm <uuid>`. NAT is the profile
    // default, so a lone `--nic1 nat` with nothing else set is still worth issuing to guarantee the
    // adapter is attached; but if the mode is NAT and no other flag was added, skip it.
    let only_nat = matches!(cfg.network_mode, NetworkMode::Nat) && modify.len() == 4;
    if modify.len() > 2 && !only_nat {
        plan.insert(
            0,
            vcmd(
                "Apply network identity and paravirtualization settings",
                modify,
            ),
        );
    }

    plan
}

/// Build commands that undo an applied configuration by restoring captured original values.
///
/// `original_extradata` maps each key from [`managed_extradata_keys`] to its captured value; a key
/// absent from the map had no value originally and is deleted. `original_modifyvm` maps each key
/// from [`managed_modifyvm_keys`] to its captured `showvminfo` value.
pub fn revert_commands(
    uuid: &str,
    profile: &VmProfile,
    original_extradata: &BTreeMap<String, String>,
    original_modifyvm: &BTreeMap<String, String>,
) -> Vec<VboxCommand> {
    let mut plan = Vec::new();

    // Restore modifyvm-level fields first (attach the adapter before anything else).
    let mut modify: Vec<String> = vec!["modifyvm".into(), uuid.into()];
    if let Some(nic) = original_modifyvm.get("nic1") {
        modify.extend(["--nic1".into(), nic.clone()]);
    }
    if let Some(t) = original_modifyvm.get("nictype1") {
        modify.extend(["--nic-type1".into(), t.clone()]);
    }
    // A captured MAC is 12 hex digits with no separators, exactly what --macaddress1 wants.
    if let Some(mac) = original_modifyvm
        .get("macaddress1")
        .filter(|m| !m.is_empty())
    {
        modify.extend(["--macaddress1".into(), mac.clone()]);
    }
    if let Some(hw) = original_modifyvm
        .get("hardwareuuid")
        .filter(|h| !h.is_empty())
    {
        modify.extend(["--hardwareuuid".into(), hw.clone()]);
    }
    if let Some(p) = original_modifyvm
        .get("paravirtprovider")
        .filter(|p| !p.is_empty())
    {
        modify.extend(["--paravirtprovider".into(), p.clone()]);
    }
    if modify.len() > 2 {
        plan.push(vcmd(
            "Restore the original network and paravirtualization settings",
            modify,
        ));
    }

    // Restore or delete each managed extra-data key.
    for key in managed_extradata_keys(profile) {
        match original_extradata.get(&key) {
            Some(v) if !v.is_empty() => plan.push(set_extra(uuid, &key, v)),
            _ => plan.push(unset_extra(uuid, &key)),
        }
    }
    plan
}

// ---------------------------------------------------------------------------
// Transparency: preview and remaining indicators
// ---------------------------------------------------------------------------

/// One human-readable line describing a change the config will make.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityEffect {
    pub area: String,
    pub change: String,
    /// Where in Windows this becomes visible, so a tester can confirm it.
    pub visible_as: String,
}

/// A preview of what a config does and what it cannot hide. Pure; used by the UI and tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityPreview {
    pub enabled: bool,
    pub effects: Vec<IdentityEffect>,
    pub connectivity_note: String,
    /// Signals that remain visible to the guest regardless of these settings. Never empty.
    pub remaining_indicators: Vec<String>,
    pub command_count: usize,
}

fn eff(area: &str, change: &str, visible_as: &str) -> IdentityEffect {
    IdentityEffect {
        area: area.into(),
        change: change.into(),
        visible_as: visible_as.into(),
    }
}

/// Indicators that stay visible no matter how identity is configured. This list is deliberately
/// conservative and is surfaced to the user so the app never implies a VM is undetectable.
pub fn remaining_indicators(cfg: &IdentityConfig) -> Vec<String> {
    let mut v = vec![
        "Guest Additions, if installed, expose VBoxService.exe, VBox kernel drivers and \\\\VirtualBox\\\\GuestInfo guest properties. Removing them defeats detection here but breaks clipboard, display resizing and network-status reporting.".to_string(),
        "The emulated GPU and monitor still report VirtualBox (VBoxSVGA / \"Oracle VirtualBox\"), visible in Device Manager and EDID.".to_string(),
        "PCI/USB device vendor and revision IDs of the emulated chipset, audio and USB controllers are VirtualBox's and are not changed by these settings.".to_string(),
        "Timing and behavioural checks (RDTSC deltas, hypervisor exits, CPUID leaf 0x40000000) can still reveal virtualization; some of these persist even with the paravirtualization interface set to none.".to_string(),
        "The ACPI tables, and various registry and driver artifacts Windows records, still carry VirtualBox-specific values.".to_string(),
    ];
    if cfg
        .branding
        .paravirt_provider
        .as_deref()
        .is_none_or(|p| p != "none")
    {
        v.push(
            "The hypervisor is still advertised through CPUID (the paravirtualization interface is not set to none), so any hypervisor-present check will succeed."
                .to_string(),
        );
    }
    if cfg.mac_address.is_none() {
        v.push(
            "The network adapter's MAC address keeps VirtualBox's 08:00:27 OUI prefix, which identifies the vendor; set a MAC to change it."
                .to_string(),
        );
    }
    v
}

/// Produce a full preview for a config against a profile.
pub fn preview(cfg: &IdentityConfig, profile: &VmProfile) -> IdentityPreview {
    let mut effects = Vec::new();
    if cfg.enabled {
        let f = &cfg.firmware;
        if f.bios_vendor.is_some() || f.bios_version.is_some() || f.bios_release_date.is_some() {
            effects.push(eff(
                "Firmware",
                "Sets the SMBIOS BIOS vendor/version/date the guest reads.",
                "msinfo32 \u{2192} BIOS Version/Date; WMIC BIOS; Get-CimInstance Win32_BIOS.",
            ));
        }
        let s = &cfg.system;
        if s.manufacturer.is_some()
            || s.product_name.is_some()
            || s.version.is_some()
            || s.serial_number.is_some()
            || s.sku.is_some()
            || s.family.is_some()
        {
            effects.push(eff(
                "System",
                "Sets the SMBIOS system manufacturer, product, version, serial, SKU and family.",
                "Get-CimInstance Win32_ComputerSystem / Win32_SystemEnclosure; msinfo32 System Manufacturer/Model.",
            ));
        }
        if s.uuid.is_some() {
            effects.push(eff(
                "System UUID",
                "Sets the SMBIOS system UUID (and the VM's hardware UUID to match).",
                "wmic csproduct get UUID; Get-CimInstance Win32_ComputerSystemProduct.",
            ));
        }
        if s.board_manufacturer.is_some() || s.board_product.is_some() || s.board_serial.is_some() {
            effects.push(eff(
                "Mainboard",
                "Sets the SMBIOS baseboard manufacturer, product and serial.",
                "Get-CimInstance Win32_BaseBoard; msinfo32 BaseBoard entries.",
            ));
        }
        if s.chassis_manufacturer.is_some() || s.chassis_asset_tag.is_some() {
            effects.push(eff(
                "Chassis",
                "Sets the SMBIOS chassis manufacturer and asset tag.",
                "Get-CimInstance Win32_SystemEnclosure.",
            ));
        }
        let st = &cfg.storage;
        if st.disk_serial.is_some()
            || st.disk_model.is_some()
            || st.disk_firmware_revision.is_some()
        {
            effects.push(eff(
                "Storage",
                "Sets the disk's ATA serial, model and firmware revision strings.",
                "wmic diskdrive get Model,SerialNumber; Get-PhysicalDisk; Get-Disk.",
            ));
        }
        if cfg.mac_address.is_some() {
            effects.push(eff(
                "Network adapter address",
                "Sets the adapter MAC address, changing the vendor OUI the guest reports.",
                "ipconfig /all; getmac; Get-NetAdapter.",
            ));
        }
        if let Some(model) = &cfg.adapter_model {
            effects.push(eff(
                "Network adapter model",
                &format!("Presents the adapter as {model}."),
                "Device Manager \u{2192} Network adapters; Get-NetAdapter.",
            ));
        }
        if !matches!(cfg.network_mode, NetworkMode::Nat) {
            effects.push(eff(
                "Network mode",
                &format!("Attaches the adapter in {} mode.", cfg.network_mode.label()),
                "The guest's IP range and reachability change accordingly.",
            ));
        }
        if let Some(p) = &cfg.branding.paravirt_provider {
            effects.push(eff(
                "Paravirtualization",
                &format!("Sets the paravirtualization interface to '{p}'."),
                "CPUID leaf 0x40000000 and the hypervisor-present bit in the guest.",
            ));
        }
        if cfg.branding.clear_vbox_oem_strings {
            effects.push(eff(
                "OEM strings",
                "Replaces the VirtualBox vboxVer_/vboxRev_ SMBIOS OEM strings with a neutral value.",
                "Get-CimInstance -Namespace root/cimv2 Win32_ComputerSystemProduct; SMBIOS type 11 dumps.",
            ));
        }
    }
    IdentityPreview {
        enabled: cfg.enabled,
        command_count: apply_commands(cfg, "PREVIEW", profile).len(),
        effects,
        connectivity_note: cfg.network_mode.connectivity_note().to_string(),
        remaining_indicators: remaining_indicators(cfg),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{profile_for, GuestArch};

    fn full_config() -> IdentityConfig {
        IdentityConfig {
            enabled: true,
            firmware: FirmwareIdentity {
                bios_vendor: Some("American Megatrends Inc.".into()),
                bios_version: Some("1503".into()),
                bios_release_date: Some("04/12/2023".into()),
            },
            system: SystemIdentity {
                manufacturer: Some("Dell Inc.".into()),
                product_name: Some("OptiPlex 7090".into()),
                version: Some("1.0".into()),
                serial_number: Some("7ABC123".into()),
                sku: Some("0A1B".into()),
                family: Some("OptiPlex".into()),
                uuid: Some("4c4c4544-0042-4310-8043-b1c04f333233".into()),
                board_manufacturer: Some("Dell Inc.".into()),
                board_product: Some("0K240Y".into()),
                board_serial: Some("/7ABC123/".into()),
                chassis_manufacturer: Some("Dell Inc.".into()),
                chassis_asset_tag: Some("ASSET-42".into()),
            },
            storage: StorageIdentity {
                disk_serial: Some("S3Z1NB0K123456".into()),
                disk_model: Some("Samsung SSD 970 EVO Plus".into()),
                disk_firmware_revision: Some("2B2QEXM7".into()),
            },
            mac_address: Some("3C:52:82:AB:CD:EF".into()),
            adapter_model: Some("82545EM".into()),
            network_mode: NetworkMode::Bridged {
                host_adapter: "en0".into(),
            },
            branding: BrandingReduction {
                paravirt_provider: Some("none".into()),
                clear_vbox_oem_strings: true,
            },
        }
    }

    fn args_pair<'a>(c: &'a VboxCommand, flag: &str) -> Option<&'a str> {
        c.args
            .windows(2)
            .find(|w| w[0] == flag)
            .map(|w| w[1].as_str())
    }

    #[test]
    fn default_is_disabled_and_produces_nothing() {
        let cfg = IdentityConfig::default();
        assert!(!cfg.enabled);
        assert!(validate(&cfg).is_empty());
        assert!(apply_commands(&cfg, "u", &profile_for(GuestArch::X64)).is_empty());
        // Even a config with fields set but disabled changes nothing.
        let mut c = full_config();
        c.enabled = false;
        assert!(apply_commands(&c, "u", &profile_for(GuestArch::X64)).is_empty());
    }

    #[test]
    fn full_config_generates_expected_commands() {
        let cfg = full_config();
        let prof = profile_for(GuestArch::X64);
        assert!(
            validate(&cfg).is_empty(),
            "should validate: {:?}",
            validate(&cfg)
        );
        let plan = apply_commands(&cfg, "uuid-9", &prof);

        // The single modifyvm step is first and carries network + paravirt + hardware uuid.
        let modify = &plan[0];
        assert_eq!(modify.args[0], "modifyvm");
        assert_eq!(args_pair(modify, "--nic1"), Some("bridged"));
        assert_eq!(args_pair(modify, "--bridge-adapter1"), Some("en0"));
        assert_eq!(args_pair(modify, "--nic-type1"), Some("82545EM"));
        assert_eq!(args_pair(modify, "--macaddress1"), Some("3C5282ABCDEF"));
        assert_eq!(
            args_pair(modify, "--hardwareuuid"),
            Some("4c4c4544-0042-4310-8043-b1c04f333233")
        );
        assert_eq!(args_pair(modify, "--paravirtprovider"), Some("none"));

        // DMI system product is set via extra data on the pcbios device.
        let dmi = plan
            .iter()
            .find(|c| c.args.iter().any(|a| a.ends_with("DmiSystemProduct")))
            .unwrap();
        assert_eq!(dmi.args[0], "setextradata");
        assert!(dmi.args.contains(&"OptiPlex 7090".to_string()));

        // Storage strings go on the ahci device at port 0.
        let serial = plan
            .iter()
            .find(|c| {
                c.args
                    .iter()
                    .any(|a| a.contains("ahci/0/Config/Port0/SerialNumber"))
            })
            .unwrap();
        assert!(serial.args.contains(&"S3Z1NB0K123456".to_string()));

        // OEM strings are neutralised, not deleted (a value is present).
        let oem = plan
            .iter()
            .find(|c| c.args.iter().any(|a| a.ends_with(OEM_VBOX_VER)))
            .unwrap();
        assert_eq!(oem.args.len(), 4);
    }

    #[test]
    fn mac_is_normalized_into_the_command() {
        let mut cfg = IdentityConfig {
            enabled: true,
            ..Default::default()
        };
        cfg.mac_address = Some("08-00-27-aa-bb-cc".into());
        let plan = apply_commands(&cfg, "u", &profile_for(GuestArch::X64));
        let modify = &plan[0];
        assert_eq!(args_pair(modify, "--macaddress1"), Some("080027AABBCC"));
    }

    #[test]
    fn nat_only_config_does_not_emit_a_pointless_modifyvm() {
        let cfg = IdentityConfig {
            enabled: true,
            ..Default::default()
        };
        // Enabled but nothing set: no commands at all.
        assert!(apply_commands(&cfg, "u", &profile_for(GuestArch::X64)).is_empty());
    }

    #[test]
    fn validation_catches_bad_values() {
        let mut cfg = full_config();
        cfg.mac_address = Some("ZZ0027AABBCC".into());
        assert!(validate(&cfg).iter().any(|p| p.contains("MAC")));

        let mut cfg = full_config();
        cfg.mac_address = Some("010027AABBCC".into()); // multicast bit set
        assert!(validate(&cfg).iter().any(|p| p.contains("multicast")));

        let mut cfg = full_config();
        cfg.system.uuid = Some("not-a-uuid".into());
        assert!(validate(&cfg).iter().any(|p| p.contains("UUID")));

        let mut cfg = full_config();
        cfg.storage.disk_serial = Some("x".repeat(21));
        assert!(validate(&cfg).iter().any(|p| p.contains("serial")));

        let mut cfg = full_config();
        cfg.adapter_model = Some("e1000".into());
        assert!(validate(&cfg).iter().any(|p| p.contains("adapter model")));

        let mut cfg = full_config();
        cfg.branding.paravirt_provider = Some("magic".into());
        assert!(validate(&cfg)
            .iter()
            .any(|p| p.contains("paravirtualization")));

        let mut cfg = full_config();
        cfg.network_mode = NetworkMode::Bridged {
            host_adapter: "  ".into(),
        };
        assert!(validate(&cfg).iter().any(|p| p.contains("Bridged")));
    }

    #[test]
    fn revert_restores_captured_values_and_deletes_the_rest() {
        let prof = profile_for(GuestArch::X64);
        let mut extradata = BTreeMap::new();
        extradata.insert(pcbios("DmiSystemProduct"), "VirtualBox".to_string());
        let mut modifyvm = BTreeMap::new();
        modifyvm.insert("nic1".to_string(), "nat".to_string());
        modifyvm.insert("nictype1".to_string(), "82540EM".to_string());
        modifyvm.insert("macaddress1".to_string(), "080027001122".to_string());

        let plan = revert_commands("u", &prof, &extradata, &modifyvm);

        // Restores the modifyvm fields.
        let modify = plan.iter().find(|c| c.args[0] == "modifyvm").unwrap();
        assert_eq!(args_pair(modify, "--nic-type1"), Some("82540EM"));
        assert_eq!(args_pair(modify, "--macaddress1"), Some("080027001122"));

        // The captured DMI key is restored to its captured value.
        let restore = plan
            .iter()
            .find(|c| c.args.iter().any(|a| a.ends_with("DmiSystemProduct")))
            .unwrap();
        assert!(restore.args.contains(&"VirtualBox".to_string()));

        // A managed key that was NOT captured is deleted (setextradata with no value => 3 args).
        let deleted = plan
            .iter()
            .find(|c| c.args.iter().any(|a| a.ends_with("DmiSystemSerial")))
            .unwrap();
        assert_eq!(deleted.args.len(), 3);
    }

    #[test]
    fn preview_always_lists_remaining_indicators() {
        let prof = profile_for(GuestArch::X64);
        let p = preview(&IdentityConfig::default(), &prof);
        assert!(!p.enabled);
        assert!(!p.remaining_indicators.is_empty());

        let p = preview(&full_config(), &prof);
        assert!(p.enabled);
        assert!(!p.effects.is_empty());
        assert!(p.command_count > 0);
        // Guest Additions caveat is always present.
        assert!(p
            .remaining_indicators
            .iter()
            .any(|i| i.contains("Guest Additions")));
        // With paravirt none and a MAC set, those specific caveats are not added.
        assert!(!p
            .remaining_indicators
            .iter()
            .any(|i| i.contains("08:00:27")));
    }

    #[test]
    fn arm_profile_uses_the_same_ahci_storage_device() {
        // The storage device key is the emulated controller, not the guest arch.
        let prof = profile_for(GuestArch::Arm64);
        assert!(storage_key(&prof, "SerialNumber").contains("ahci/0/Config/Port0/SerialNumber"));
    }
}
