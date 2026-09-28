//! Architecture-specific, version-aware VM configuration profiles and resource sizing.
//!
//! Sources for the defaults (verified against the VirtualBox 7.2 source tree,
//! `src/VBox/Main/src-all/Global.cpp`, and the 7.2 user guide "Arm Host Limitations"):
//! - Windows11_64 (x86_64 guest): EFI + Secure Boot + TPM 2.0, VBoxSVGA graphics, Intel 82540EM NIC,
//!   Intel AHCI SATA storage, PIIX3 chipset.
//! - Windows11_arm64 (Arm guest): EFI (mandatory for Arm), Secure Boot, VBoxSVGA graphics,
//!   "UsbNet" (USB NCM) network adapter by default, Intel AHCI SATA storage, ARMv8 virtual chipset.
//!   Arm hosts run only Arm guests; unattended installation is not available on Arm hosts.

use serde::{Deserialize, Serialize};

use crate::host::{Arch, HostInfo, HostOs, Tri};

pub const GIB: u64 = 1024 * 1024 * 1024;
pub const MIB: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuestArch {
    X64,
    Arm64,
}

impl GuestArch {
    pub fn for_host(arch: Arch) -> Option<GuestArch> {
        match arch {
            Arch::X86_64 => Some(GuestArch::X64),
            Arch::Aarch64 => Some(GuestArch::Arm64),
            Arch::Other => None,
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            GuestArch::X64 => "Windows 11 (x64)",
            GuestArch::Arm64 => "Windows 11 on ARM (ARM64)",
        }
    }
    pub fn microsoft_download_page(&self) -> &'static str {
        match self {
            GuestArch::X64 => "https://www.microsoft.com/software-download/windows11",
            GuestArch::Arm64 => "https://www.microsoft.com/software-download/windows11arm64",
        }
    }
}

/// How much this host/guest combination has actually been validated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportLevel {
    /// Full end-to-end path exercised on real hardware by the project.
    Tested,
    /// Implemented from official documentation; not yet exercised end-to-end by the project.
    Untested,
    /// VirtualBox itself calls this experimental (e.g. Windows on Arm hosts).
    Experimental,
    /// Not supported by VirtualBox or by this app.
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VmProfile {
    pub guest_arch: GuestArch,
    /// VBoxManage `--ostype` id.
    pub ostype_id: String,
    /// VBoxManage `--platform-architecture` value.
    pub platform_architecture: String,
    pub firmware: String,
    pub secure_boot: bool,
    /// `--tpm-type` value, when the app sets it explicitly.
    pub tpm_type: Option<String>,
    pub graphics_controller: String,
    pub vram_mb: u32,
    /// Default NIC emulation (`--nic-type1`).
    pub nic_type: String,
    /// Alternative NIC emulation used by the network-recovery path.
    pub recovery_nic_type: String,
    pub storage_controller_name: String,
    pub storage_bus: String,
    pub storage_controller_type: String,
    pub chipset: String,
    pub min_virtualbox_version: String,
    /// Guest Additions installer file name inside VBoxGuestAdditions.iso.
    pub guest_additions_installer: String,
    /// Whether `VBoxManage unattended` is available for this guest on its host.
    pub unattended_available: bool,
}

pub fn profile_for(arch: GuestArch) -> VmProfile {
    match arch {
        GuestArch::X64 => VmProfile {
            guest_arch: arch,
            ostype_id: "Windows11_64".into(),
            platform_architecture: "x86".into(),
            firmware: "efi".into(),
            secure_boot: true,
            tpm_type: Some("2.0".into()),
            graphics_controller: "vboxsvga".into(),
            vram_mb: 128,
            nic_type: "82540EM".into(),
            recovery_nic_type: "virtio".into(),
            storage_controller_name: "SATA".into(),
            storage_bus: "sata".into(),
            storage_controller_type: "IntelAhci".into(),
            chipset: "piix3".into(),
            min_virtualbox_version: "7.0.0".into(),
            guest_additions_installer: "VBoxWindowsAdditions.exe".into(),
            unattended_available: true,
        },
        GuestArch::Arm64 => VmProfile {
            guest_arch: arch,
            ostype_id: "Windows11_arm64".into(),
            platform_architecture: "arm".into(),
            firmware: "efi".into(),
            secure_boot: true,
            // The Arm "System" settings page has no TPM control; `createvm --default` applies Oracle's
            // own defaults for the OS type, so the app does not force a TPM setting on Arm.
            tpm_type: None,
            graphics_controller: "vboxsvga".into(),
            vram_mb: 128,
            nic_type: "usbnet".into(),
            recovery_nic_type: "virtio".into(),
            storage_controller_name: "SATA".into(),
            storage_bus: "sata".into(),
            storage_controller_type: "IntelAhci".into(),
            chipset: "armv8virtual".into(),
            min_virtualbox_version: "7.2.0".into(),
            guest_additions_installer: "VBoxWindowsAdditions-arm64.exe".into(),
            unattended_available: false,
        },
    }
}

/// Support level of a host combination, kept honest with docs/COMPATIBILITY.md.
pub fn support_level(os: HostOs, arch: Arch) -> SupportLevel {
    match (os, arch) {
        (HostOs::MacOs, Arch::Aarch64) => SupportLevel::Untested,
        (HostOs::Windows, Arch::X86_64) => SupportLevel::Untested,
        (HostOs::MacOs, Arch::X86_64) => SupportLevel::Untested,
        (HostOs::Windows, Arch::Aarch64) => SupportLevel::Experimental,
        _ => SupportLevel::Unsupported,
    }
}

// ---------- resource sizing ----------

pub const WIN11_MIN_RAM_MB: u32 = 4096;
pub const WIN11_MIN_CPUS: u32 = 2;
pub const WIN11_MIN_DISK_GB: u32 = 64;
pub const DEFAULT_DISK_GB: u32 = 100;
pub const MAX_DISK_GB: u32 = 2000;
pub const RECOMMENDED_RAM_MB: u32 = 8192;
pub const RECOMMENDED_CPUS: u32 = 4;

/// Reserve for the host: at least 4 GiB, or 25 % of RAM on bigger machines.
pub fn host_ram_reserve_bytes(total: u64) -> u64 {
    (4 * GIB).max(total / 4)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceLimits {
    pub min_ram_mb: u32,
    pub max_ram_mb: u32,
    pub recommended_ram_mb: u32,
    pub min_cpus: u32,
    pub max_cpus: u32,
    pub recommended_cpus: u32,
    pub min_disk_gb: u32,
    pub max_disk_gb: u32,
    pub recommended_disk_gb: u32,
}

/// Conservative sizing that always leaves the host enough to keep working.
pub fn resource_limits(host: &HostInfo) -> ResourceLimits {
    let total = host.total_ram_bytes;
    let usable = total.saturating_sub(host_ram_reserve_bytes(total));
    let max_ram_mb = ((usable / MIB) / 512 * 512) as u32; // round down to 512 MiB
    let recommended_ram_mb = RECOMMENDED_RAM_MB.min(max_ram_mb).max(0);
    let cpus = host.logical_cpus as u32;
    let max_cpus = cpus.saturating_sub(1).max(1);
    let recommended_cpus = RECOMMENDED_CPUS.min((cpus / 2).max(WIN11_MIN_CPUS)).min(max_cpus);
    ResourceLimits {
        min_ram_mb: WIN11_MIN_RAM_MB,
        max_ram_mb,
        recommended_ram_mb,
        min_cpus: WIN11_MIN_CPUS,
        max_cpus,
        recommended_cpus,
        min_disk_gb: WIN11_MIN_DISK_GB,
        max_disk_gb: MAX_DISK_GB,
        recommended_disk_gb: DEFAULT_DISK_GB,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VmSizing {
    pub ram_mb: u32,
    pub cpus: u32,
    pub disk_gb: u32,
}

pub fn validate_sizing(s: &VmSizing, limits: &ResourceLimits) -> Vec<String> {
    let mut problems = Vec::new();
    if s.ram_mb < limits.min_ram_mb {
        problems.push(format!("Windows 11 needs at least {} MB of memory.", limits.min_ram_mb));
    }
    if s.ram_mb > limits.max_ram_mb {
        problems.push(format!(
            "Giving Windows {} MB would not leave enough memory for this computer (maximum {} MB).",
            s.ram_mb, limits.max_ram_mb
        ));
    }
    if s.cpus < limits.min_cpus {
        problems.push(format!("Windows 11 needs at least {} processors.", limits.min_cpus));
    }
    if s.cpus > limits.max_cpus {
        problems.push(format!("Leave at least one processor for this computer (maximum {}).", limits.max_cpus));
    }
    if s.disk_gb < limits.min_disk_gb {
        problems.push(format!("Windows 11 needs a disk of at least {} GB.", limits.min_disk_gb));
    }
    if s.disk_gb > limits.max_disk_gb {
        problems.push(format!("The virtual disk cannot be larger than {} GB.", limits.max_disk_gb));
    }
    problems
}

// ---------- storage estimates ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageEstimate {
    pub windows_iso_bytes: u64,
    pub virtualbox_installer_bytes: u64,
    pub initial_windows_install_bytes: u64,
    pub working_space_bytes: u64,
    pub total_bytes: u64,
}

/// Immediate free-space requirement to get through installation (not the disk's maximum size).
pub fn storage_estimate(guest: GuestArch, iso_already_present: bool) -> StorageEstimate {
    let windows_iso_bytes = if iso_already_present { 0 } else { 7 * GIB };
    let virtualbox_installer_bytes = match guest {
        GuestArch::X64 => 400 * MIB,
        GuestArch::Arm64 => 250 * MIB,
    };
    let initial_windows_install_bytes = 30 * GIB;
    let working_space_bytes = 8 * GIB;
    let total_bytes =
        windows_iso_bytes + virtualbox_installer_bytes + initial_windows_install_bytes + working_space_bytes;
    StorageEstimate {
        windows_iso_bytes,
        virtualbox_installer_bytes,
        initial_windows_install_bytes,
        working_space_bytes,
        total_bytes,
    }
}

// ---------- assessment ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Blocker,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Finding {
    pub code: String,
    pub severity: Severity,
    pub title: String,
    pub detail: String,
    pub action: Option<String>,
}

impl Finding {
    fn new(code: &str, severity: Severity, title: &str, detail: String, action: Option<&str>) -> Self {
        Finding {
            code: code.into(),
            severity,
            title: title.into(),
            detail,
            action: action.map(String::from),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assessment {
    pub guest_arch: Option<GuestArch>,
    pub support_level: SupportLevel,
    pub limits: Option<ResourceLimits>,
    pub findings: Vec<Finding>,
}

impl Assessment {
    pub fn has_blockers(&self) -> bool {
        self.findings.iter().any(|f| f.severity == Severity::Blocker)
    }
}

/// Turn host facts into plain-language findings, separating real blockers from advice.
pub fn assess(host: &HostInfo, free_at_vm_base: Option<u64>, iso_present: bool) -> Assessment {
    let mut findings = Vec::new();
    let guest = GuestArch::for_host(host.arch);
    let level = support_level(host.os, host.arch);

    match level {
        SupportLevel::Unsupported => findings.push(Finding::new(
            "host_unsupported",
            Severity::Blocker,
            "This computer type is not supported",
            format!(
                "This app supports Windows PCs with Intel/AMD processors and Macs (Apple Silicon or Intel). Detected: {} on {}.",
                host.os_name,
                host.arch.label()
            ),
            None,
        )),
        SupportLevel::Experimental => findings.push(Finding::new(
            "host_experimental",
            Severity::Blocker,
            "Windows on ARM computers are not supported yet",
            "Oracle describes VirtualBox on Windows/Arm hosts as experimental. This app does not set up VMs on it until that changes.".into(),
            None,
        )),
        _ => {}
    }

    if host.translated {
        findings.push(Finding::new(
            "app_translated",
            Severity::Warning,
            "You are running the wrong download for this computer",
            format!(
                "This app build is for {} but the computer is {}. It still works, but the native download is faster.",
                host.app_arch.label(),
                host.arch.label()
            ),
            Some("Download the build that matches this computer from the releases page."),
        ));
    }

    if host.virtualization == Tri::No {
        findings.push(Finding::new(
            "virtualization_off",
            Severity::Blocker,
            "Hardware virtualization is turned off",
            "VirtualBox needs the processor's virtualization feature (Intel VT-x / AMD-V). It is currently disabled, usually in the computer's firmware (BIOS/UEFI) settings.".into(),
            Some("Enable virtualization in your computer's firmware settings, then run this check again."),
        ));
    }

    if host.hypervisor_conflict == Tri::Yes {
        findings.push(Finding::new(
            "hypervisor_present",
            Severity::Warning,
            "Another virtualization feature is active on this PC",
            format!(
                "{}. VirtualBox still works alongside it, but Windows in the VM will run noticeably slower. This app will not change these security settings for you.",
                host.hypervisor_conflict_detail.clone().unwrap_or_else(|| "Hyper-V or a related Windows feature is running".into())
            ),
            Some("If performance is poor, see the troubleshooting guide for the trade-offs before changing Windows features."),
        ));
    }

    let limits = if level != SupportLevel::Unsupported { Some(resource_limits(host)) } else { None };
    if let Some(l) = &limits {
        if l.max_ram_mb < WIN11_MIN_RAM_MB {
            findings.push(Finding::new(
                "ram_too_low",
                Severity::Blocker,
                "Not enough memory",
                format!(
                    "This computer has {:.1} GB of memory. Windows 11 needs 4 GB for itself and this computer needs at least 4 GB to keep running.",
                    host.total_ram_bytes as f64 / GIB as f64
                ),
                None,
            ));
        } else if l.recommended_ram_mb < RECOMMENDED_RAM_MB {
            findings.push(Finding::new(
                "ram_tight",
                Severity::Warning,
                "Memory is tight",
                format!(
                    "This computer has {:.1} GB of memory, so Windows will get {} MB instead of the recommended 8 GB. It will work, but expect it to feel slow.",
                    host.total_ram_bytes as f64 / GIB as f64,
                    l.recommended_ram_mb
                ),
                None,
            ));
        }
        if l.max_cpus < WIN11_MIN_CPUS {
            findings.push(Finding::new(
                "cpus_too_low",
                Severity::Blocker,
                "Not enough processor cores",
                format!("Windows 11 needs 2 processors, and one must stay free for this computer. Detected {} logical processors.", host.logical_cpus),
                None,
            ));
        } else if l.recommended_cpus < RECOMMENDED_CPUS {
            findings.push(Finding::new(
                "cpus_tight",
                Severity::Warning,
                "Few processor cores",
                format!("Windows will get {} processors out of {}. It will work but may feel slow.", l.recommended_cpus, host.logical_cpus),
                None,
            ));
        }
    }

    if let (Some(g), Some(free)) = (guest, free_at_vm_base) {
        let est = storage_estimate(g, iso_present);
        if free < est.total_bytes {
            findings.push(Finding::new(
                "disk_space",
                Severity::Blocker,
                "Not enough free disk space",
                format!(
                    "{:.1} GB is free where the VM would be stored, but about {:.0} GB is needed right now (Windows download {:.0} GB, VirtualBox {:.1} GB, initial Windows install {:.0} GB, working space {:.0} GB). The virtual disk grows later as you use Windows.",
                    free as f64 / GIB as f64,
                    est.total_bytes as f64 / GIB as f64,
                    est.windows_iso_bytes as f64 / GIB as f64,
                    est.virtualbox_installer_bytes as f64 / GIB as f64,
                    est.initial_windows_install_bytes as f64 / GIB as f64,
                    est.working_space_bytes as f64 / GIB as f64
                ),
                Some("Free up space, or choose a different storage location in Advanced settings."),
            ));
        }
    }

    Assessment {
        guest_arch: guest,
        support_level: level,
        limits,
        findings,
    }
}

// ---------- VirtualBox version handling ----------

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct VboxVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub build: Option<u32>,
}

impl VboxVersion {
    /// Parses "7.2.20r175154", "7.2.20", "7.2.20_BETA1r123".
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let (ver, build) = match s.find('r') {
            Some(i) => (&s[..i], s[i + 1..].parse().ok()),
            None => (s, None),
        };
        let ver = ver.split('_').next()?;
        let mut it = ver.split('.').map(|p| p.parse::<u32>());
        let major = it.next()?.ok()?;
        let minor = it.next()?.ok()?;
        let patch = it.next().unwrap_or(Ok(0)).ok()?;
        Some(Self { major, minor, patch, build })
    }
    pub fn at_least(&self, other: &str) -> bool {
        match Self::parse(other) {
            Some(o) => (self.major, self.minor, self.patch) >= (o.major, o.minor, o.patch),
            None => false,
        }
    }
}

impl std::fmt::Display for VboxVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(b) = self.build {
            write!(f, "r{b}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(os: HostOs, arch: Arch, ram_gb: u64, cpus: usize) -> HostInfo {
        HostInfo {
            os,
            os_name: "Test".into(),
            os_version: "1".into(),
            arch,
            app_arch: arch,
            translated: false,
            total_ram_bytes: ram_gb * GIB,
            available_ram_bytes: ram_gb * GIB / 2,
            logical_cpus: cpus,
            cpu_brand: String::new(),
            virtualization: Tri::Yes,
            hypervisor_conflict: Tri::No,
            hypervisor_conflict_detail: None,
            hostname_hash: String::new(),
        }
    }

    #[test]
    fn profiles_are_architecture_specific() {
        let x = profile_for(GuestArch::X64);
        let a = profile_for(GuestArch::Arm64);
        assert_eq!(x.ostype_id, "Windows11_64");
        assert_eq!(a.ostype_id, "Windows11_arm64");
        assert_eq!(a.platform_architecture, "arm");
        assert_eq!(a.nic_type, "usbnet");
        assert_eq!(x.nic_type, "82540EM");
        assert!(a.tpm_type.is_none());
        assert_eq!(x.tpm_type.as_deref(), Some("2.0"));
        assert!(!a.unattended_available);
        assert_eq!(a.chipset, "armv8virtual");
        assert_eq!(a.min_virtualbox_version, "7.2.0");
    }

    #[test]
    fn sizing_matches_reference_mac() {
        // The manually set up Apple Silicon Mac: 24 GB RAM, 12 cores -> 8 GB / 4 CPUs.
        let h = host(HostOs::MacOs, Arch::Aarch64, 24, 12);
        let l = resource_limits(&h);
        assert_eq!(l.recommended_ram_mb, 8192);
        assert_eq!(l.recommended_cpus, 4);
        assert_eq!(l.max_ram_mb, 18 * 1024);
        assert_eq!(l.max_cpus, 11);
    }

    #[test]
    fn sizing_is_conservative_on_small_hosts() {
        let h = host(HostOs::Windows, Arch::X86_64, 8, 4);
        let l = resource_limits(&h);
        assert_eq!(l.max_ram_mb, 4096);
        assert_eq!(l.recommended_ram_mb, 4096);
        assert_eq!(l.recommended_cpus, 2);
        let a = assess(&h, Some(500 * GIB), false);
        assert!(!a.has_blockers());
        assert!(a.findings.iter().any(|f| f.code == "ram_tight"));
        assert!(a.findings.iter().any(|f| f.code == "cpus_tight"));
    }

    #[test]
    fn too_small_host_is_blocked() {
        let h = host(HostOs::Windows, Arch::X86_64, 6, 2);
        let a = assess(&h, Some(500 * GIB), false);
        assert!(a.has_blockers());
        assert!(a.findings.iter().any(|f| f.code == "ram_too_low"));
        assert!(a.findings.iter().any(|f| f.code == "cpus_too_low"));
    }

    #[test]
    fn unsupported_combinations() {
        let a = assess(&host(HostOs::Windows, Arch::Aarch64, 16, 8), Some(500 * GIB), false);
        assert_eq!(a.support_level, SupportLevel::Experimental);
        assert!(a.has_blockers());
        let a = assess(&host(HostOs::Linux, Arch::X86_64, 16, 8), Some(500 * GIB), false);
        assert_eq!(a.support_level, SupportLevel::Unsupported);
        assert!(a.has_blockers());
    }

    #[test]
    fn disk_blocker_reports_amounts() {
        let a = assess(&host(HostOs::MacOs, Arch::Aarch64, 16, 8), Some(20 * GIB), false);
        let f = a.findings.iter().find(|f| f.code == "disk_space").unwrap();
        assert_eq!(f.severity, Severity::Blocker);
        assert!(f.detail.contains("20.0 GB is free"));
        assert!(f.action.is_some());
        // With the ISO already present the requirement drops by 7 GB.
        let e1 = storage_estimate(GuestArch::Arm64, false);
        let e2 = storage_estimate(GuestArch::Arm64, true);
        assert_eq!(e1.total_bytes - e2.total_bytes, 7 * GIB);
    }

    #[test]
    fn virtualization_off_is_blocker_and_hyperv_is_warning() {
        let mut h = host(HostOs::Windows, Arch::X86_64, 32, 16);
        h.virtualization = Tri::No;
        h.hypervisor_conflict = Tri::Yes;
        let a = assess(&h, Some(500 * GIB), false);
        let v = a.findings.iter().find(|f| f.code == "virtualization_off").unwrap();
        assert_eq!(v.severity, Severity::Blocker);
        let c = a.findings.iter().find(|f| f.code == "hypervisor_present").unwrap();
        assert_eq!(c.severity, Severity::Warning);
    }

    #[test]
    fn sizing_validation() {
        let l = resource_limits(&host(HostOs::MacOs, Arch::Aarch64, 16, 8));
        assert!(validate_sizing(&VmSizing { ram_mb: 8192, cpus: 4, disk_gb: 100 }, &l).is_empty());
        assert!(!validate_sizing(&VmSizing { ram_mb: 2048, cpus: 4, disk_gb: 100 }, &l).is_empty());
        assert!(!validate_sizing(&VmSizing { ram_mb: 8192, cpus: 8, disk_gb: 100 }, &l).is_empty());
        assert!(!validate_sizing(&VmSizing { ram_mb: 8192, cpus: 4, disk_gb: 20 }, &l).is_empty());
    }

    #[test]
    fn version_parse() {
        let v = VboxVersion::parse("7.2.20r175154").unwrap();
        assert_eq!((v.major, v.minor, v.patch, v.build), (7, 2, 20, Some(175154)));
        assert!(v.at_least("7.2.0"));
        assert!(!v.at_least("7.3.0"));
        assert_eq!(VboxVersion::parse("7.1.4_BETA1r162349").unwrap().patch, 4);
        assert!(VboxVersion::parse("garbage").is_none());
        assert_eq!(v.to_string(), "7.2.20r175154");
    }
}
