//! Host inspection: what kind of computer is this, and can it run a Windows 11 VM?
//!
//! Three architectures are deliberately kept apart everywhere in this crate:
//! - the architecture of *this application binary* (`APP_ARCH`),
//! - the *real* host processor architecture (`HostInfo::arch`), even when the app runs
//!   under Rosetta 2 or Windows x64 emulation,
//! - the guest architecture (see `profile::GuestArch`).

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::cmd;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostOs {
    Windows,
    MacOs,
    Linux,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arch {
    X86_64,
    Aarch64,
    Other,
}

impl Arch {
    pub fn from_str_loose(s: &str) -> Arch {
        match s.trim().to_ascii_lowercase().as_str() {
            "x86_64" | "amd64" | "x64" => Arch::X86_64,
            "aarch64" | "arm64" | "arm64e" => Arch::Aarch64,
            _ => Arch::Other,
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Arch::X86_64 => "Intel/AMD 64-bit (x86_64)",
            Arch::Aarch64 => "ARM 64-bit (Apple Silicon / Arm64)",
            Arch::Other => "Unknown",
        }
    }
}

/// Architecture this binary was compiled for.
pub const APP_ARCH: Arch = if cfg!(target_arch = "x86_64") {
    Arch::X86_64
} else if cfg!(target_arch = "aarch64") {
    Arch::Aarch64
} else {
    Arch::Other
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tri {
    Yes,
    No,
    Unknown,
}

impl From<Option<bool>> for Tri {
    fn from(v: Option<bool>) -> Self {
        match v {
            Some(true) => Tri::Yes,
            Some(false) => Tri::No,
            None => Tri::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskSpace {
    pub path: PathBuf,
    /// Mount point / volume that contains `path` (or the nearest existing ancestor).
    pub mount_point: PathBuf,
    pub available_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostInfo {
    pub os: HostOs,
    pub os_name: String,
    pub os_version: String,
    /// Real processor architecture, not the app's.
    pub arch: Arch,
    pub app_arch: Arch,
    /// True when the app runs under translation (Rosetta 2 or Windows x64-on-Arm emulation).
    pub translated: bool,
    pub total_ram_bytes: u64,
    pub available_ram_bytes: u64,
    pub logical_cpus: usize,
    pub cpu_brand: String,
    /// Hardware virtualization appears available to VirtualBox.
    pub virtualization: Tri,
    /// Windows: Hyper-V / Virtual Machine Platform / Core Isolation (HVCI) engaged.
    /// VirtualBox 7 can coexist with these but runs slower; this is a recommendation, not a blocker.
    pub hypervisor_conflict: Tri,
    pub hypervisor_conflict_detail: Option<String>,
    pub hostname_hash: String,
}

impl HostInfo {
    pub fn is_supported_host(&self) -> bool {
        matches!(
            (self.os, self.arch),
            (HostOs::Windows, Arch::X86_64)
                | (HostOs::MacOs, Arch::Aarch64)
                | (HostOs::MacOs, Arch::X86_64)
        )
    }
}

/// Inspect the host. Never fails outright: unknown facts are reported as `Tri::Unknown`.
pub async fn inspect() -> HostInfo {
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_list(sysinfo::CpuRefreshKind::nothing());

    let os = current_os();
    let os_name = System::name().unwrap_or_else(|| "Unknown".into());
    let os_version = System::os_version()
        .or_else(System::kernel_version)
        .unwrap_or_default();
    let (arch, translated) = real_arch().await;
    let cpu_brand = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .unwrap_or_default();
    let logical_cpus = sys.cpus().len().max(1);
    let (virtualization, hypervisor_conflict, detail) = virtualization_status(os, arch).await;
    let hostname = System::host_name().unwrap_or_default();
    let hostname_hash = {
        use sha2::Digest;
        hex::encode(&sha2::Sha256::digest(hostname.as_bytes())[..6])
    };

    HostInfo {
        os,
        os_name,
        os_version,
        arch,
        app_arch: APP_ARCH,
        translated,
        total_ram_bytes: sys.total_memory(),
        available_ram_bytes: sys.available_memory(),
        logical_cpus,
        cpu_brand,
        virtualization,
        hypervisor_conflict,
        hypervisor_conflict_detail: detail,
        hostname_hash,
    }
}

pub fn current_os() -> HostOs {
    if cfg!(target_os = "windows") {
        HostOs::Windows
    } else if cfg!(target_os = "macos") {
        HostOs::MacOs
    } else if cfg!(target_os = "linux") {
        HostOs::Linux
    } else {
        HostOs::Other
    }
}

/// Free space on the volume that holds `path` (or its nearest existing ancestor).
pub fn disk_space(path: &Path) -> Option<DiskSpace> {
    use sysinfo::Disks;
    let probe = nearest_existing_ancestor(path)?;
    let canonical = strip_verbatim_prefix(std::fs::canonicalize(&probe).unwrap_or(probe.clone()));
    let disks = Disks::new_with_refreshed_list();
    let mut best: Option<(&sysinfo::Disk, usize)> = None;
    for d in disks.list() {
        let mp = d.mount_point();
        if path_starts_with(&canonical, mp) {
            let len = mp.as_os_str().len();
            if best.map(|(_, l)| len > l).unwrap_or(true) {
                best = Some((d, len));
            }
        }
    }
    let (d, _) = best?;
    Some(DiskSpace {
        path: path.to_path_buf(),
        mount_point: d.mount_point().to_path_buf(),
        available_bytes: d.available_space(),
        total_bytes: d.total_space(),
    })
}

/// Windows `canonicalize` returns `\\?\C:\...`; mount points are reported as `C:\`.
fn strip_verbatim_prefix(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        return PathBuf::from(rest);
    }
    p
}

fn path_starts_with(path: &Path, prefix: &Path) -> bool {
    // Case-insensitive on Windows drive letters ("C:\" vs "c:\").
    let p = path.to_string_lossy();
    let q = prefix.to_string_lossy();
    if cfg!(windows) {
        p.to_ascii_lowercase().starts_with(&q.to_ascii_lowercase())
    } else {
        path.starts_with(prefix)
    }
}

pub fn nearest_existing_ancestor(path: &Path) -> Option<PathBuf> {
    let mut cur = Some(path.to_path_buf());
    while let Some(p) = cur {
        if p.exists() {
            return Some(p);
        }
        cur = p.parent().map(|x| x.to_path_buf());
    }
    None
}

// ---------- architecture detection ----------

async fn real_arch() -> (Arch, bool) {
    #[cfg(target_os = "macos")]
    {
        // sysctl.proc_translated == 1 means this process runs under Rosetta 2.
        let translated = sysctl_int("sysctl.proc_translated").await == Some(1);
        // hw.optional.arm64 == 1 on Apple Silicon regardless of translation.
        let arm = sysctl_int("hw.optional.arm64").await == Some(1);
        let arch = if arm { Arch::Aarch64 } else { APP_ARCH };
        return (arch, translated);
    }
    #[cfg(target_os = "windows")]
    {
        return windows_native_arch();
    }
    #[allow(unreachable_code)]
    {
        let a = std::env::consts::ARCH;
        (Arch::from_str_loose(a), false)
    }
}

#[cfg(target_os = "macos")]
async fn sysctl_int(key: &str) -> Option<i64> {
    let out = cmd::run(
        Path::new("/usr/sbin/sysctl"),
        &["-n".to_string(), key.to_string()],
        Duration::from_secs(5),
        None,
        None,
    )
    .await
    .ok()?;
    out.stdout.trim().parse().ok()
}

#[cfg(target_os = "windows")]
fn windows_native_arch() -> (Arch, bool) {
    // IsWow64Process2 reports the native machine even when this process is emulated.
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn IsWow64Process2(
            process: isize,
            process_machine: *mut u16,
            native_machine: *mut u16,
        ) -> i32;
    }
    const IMAGE_FILE_MACHINE_UNKNOWN: u16 = 0;
    const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;
    const IMAGE_FILE_MACHINE_ARM64: u16 = 0xAA64;
    let mut process_machine: u16 = 0;
    let mut native_machine: u16 = 0;
    let ok = unsafe {
        IsWow64Process2(
            GetCurrentProcess(),
            &mut process_machine,
            &mut native_machine,
        )
    };
    if ok != 0 {
        let arch = match native_machine {
            IMAGE_FILE_MACHINE_AMD64 => Arch::X86_64,
            IMAGE_FILE_MACHINE_ARM64 => Arch::Aarch64,
            _ => Arch::Other,
        };
        // process_machine is UNKNOWN when the process is native.
        let translated = process_machine != IMAGE_FILE_MACHINE_UNKNOWN && arch != APP_ARCH;
        return (arch, translated);
    }
    (Arch::from_str_loose(std::env::consts::ARCH), false)
}

// ---------- virtualization / hypervisor status ----------

async fn virtualization_status(os: HostOs, _arch: Arch) -> (Tri, Tri, Option<String>) {
    match os {
        HostOs::MacOs => {
            #[cfg(target_os = "macos")]
            {
                let hv = sysctl_int("kern.hv_support").await;
                return (Tri::from(hv.map(|v| v == 1)), Tri::No, None);
            }
            #[allow(unreachable_code)]
            (Tri::Unknown, Tri::Unknown, None)
        }
        HostOs::Windows => windows_virtualization().await,
        HostOs::Linux => {
            let flags = tokio::fs::read_to_string("/proc/cpuinfo")
                .await
                .unwrap_or_default();
            let has = flags.contains(" vmx") || flags.contains(" svm");
            (Tri::from(Some(has)), Tri::Unknown, None)
        }
        HostOs::Other => (Tri::Unknown, Tri::Unknown, None),
    }
}

async fn windows_virtualization() -> (Tri, Tri, Option<String>) {
    if !cfg!(target_os = "windows") {
        return (Tri::Unknown, Tri::Unknown, None);
    }
    // One PowerShell call, JSON output, no user-controlled input.
    let script = r#"$ErrorActionPreference='SilentlyContinue'; $p=Get-CimInstance Win32_Processor | Select-Object -First 1; $dg=Get-CimInstance -Namespace root\Microsoft\Windows\DeviceGuard -ClassName Win32_DeviceGuard; $hv=Get-CimInstance Win32_ComputerSystem; $svc=Get-Service vmcompute,HvHost -ErrorAction SilentlyContinue | Where-Object Status -eq 'Running' | Select-Object -ExpandProperty Name; [pscustomobject]@{ vt=$p.VirtualizationFirmwareEnabled; vmm=$p.VMMonitorModeExtensions; hvPresent=$hv.HypervisorPresent; secRunning=@($dg.SecurityServicesRunning); vbsStatus=$dg.VirtualizationBasedSecurityStatus; hvServices=@($svc) } | ConvertTo-Json -Compress"#;
    let ps = PathBuf::from("powershell.exe");
    let out = cmd::run(
        &ps,
        &[
            "-NoProfile".into(),
            "-NonInteractive".into(),
            "-ExecutionPolicy".into(),
            "Bypass".into(),
            "-Command".into(),
            script.into(),
        ],
        Duration::from_secs(25),
        None,
        None,
    )
    .await;
    let Ok(out) = out else {
        return (Tri::Unknown, Tri::Unknown, None);
    };
    parse_windows_virt_json(&out.stdout)
}

/// Parse the JSON produced by `windows_virtualization`. Separated for testing.
pub fn parse_windows_virt_json(json: &str) -> (Tri, Tri, Option<String>) {
    let v: serde_json::Value = match serde_json::from_str(json.trim()) {
        Ok(v) => v,
        Err(_) => return (Tri::Unknown, Tri::Unknown, None),
    };
    let vt = v.get("vt").and_then(|x| x.as_bool());
    let vmm = v.get("vmm").and_then(|x| x.as_bool());
    let hv_present = v
        .get("hvPresent")
        .and_then(|x| x.as_bool())
        .unwrap_or(false);
    let sec_running: Vec<u64> = v
        .get("secRunning")
        .and_then(|x| x.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_u64()).collect())
        .unwrap_or_default();
    let vbs_status = v.get("vbsStatus").and_then(|x| x.as_u64()).unwrap_or(0);
    let hv_services: Vec<String> = v
        .get("hvServices")
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Firmware VT can read false while a hypervisor owns the CPU; treat "hypervisor present"
    // as evidence that virtualization exists.
    let virt = match (vt, vmm) {
        (Some(true), _) | (_, Some(true)) => Tri::Yes,
        _ if hv_present => Tri::Yes,
        (Some(false), Some(false)) => Tri::No,
        _ => Tri::Unknown,
    };
    let hvci = sec_running.contains(&2);
    let cred_guard = sec_running.contains(&1);
    let conflict = hv_present || hvci || vbs_status == 2 || !hv_services.is_empty();
    let mut details = Vec::new();
    if hv_present {
        details.push("Windows reports a hypervisor is running (Hyper-V, WSL 2, Windows Sandbox or Virtual Machine Platform)".to_string());
    }
    if hvci {
        details.push("Core isolation / Memory integrity (HVCI) is on".to_string());
    }
    if cred_guard {
        details.push("Credential Guard is on".to_string());
    }
    if !hv_services.is_empty() {
        details.push(format!(
            "Hyper-V services running: {}",
            hv_services.join(", ")
        ));
    }
    (
        virt,
        Tri::from(Some(conflict)),
        if details.is_empty() {
            None
        } else {
            Some(details.join("; "))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arch_parsing() {
        assert_eq!(Arch::from_str_loose("arm64"), Arch::Aarch64);
        assert_eq!(Arch::from_str_loose("AMD64"), Arch::X86_64);
        assert_eq!(Arch::from_str_loose("x86_64"), Arch::X86_64);
        assert_eq!(Arch::from_str_loose("i686"), Arch::Other);
    }

    #[test]
    fn windows_virt_json_hyperv_on() {
        let j = r#"{"vt":true,"vmm":true,"hvPresent":true,"secRunning":[2],"vbsStatus":2,"hvServices":["vmcompute"]}"#;
        let (v, c, d) = parse_windows_virt_json(j);
        assert_eq!(v, Tri::Yes);
        assert_eq!(c, Tri::Yes);
        assert!(d.unwrap().contains("Memory integrity"));
    }

    #[test]
    fn windows_virt_json_clean() {
        let j = r#"{"vt":true,"vmm":true,"hvPresent":false,"secRunning":[],"vbsStatus":0,"hvServices":[]}"#;
        let (v, c, d) = parse_windows_virt_json(j);
        assert_eq!(v, Tri::Yes);
        assert_eq!(c, Tri::No);
        assert!(d.is_none());
    }

    #[test]
    fn windows_virt_json_disabled_in_firmware() {
        let j = r#"{"vt":false,"vmm":false,"hvPresent":false,"secRunning":[],"vbsStatus":0,"hvServices":[]}"#;
        let (v, _, _) = parse_windows_virt_json(j);
        assert_eq!(v, Tri::No);
    }

    #[test]
    fn windows_virt_json_garbage() {
        let (v, c, _) = parse_windows_virt_json("not json");
        assert_eq!(v, Tri::Unknown);
        assert_eq!(c, Tri::Unknown);
    }

    #[test]
    fn supported_hosts() {
        let mut h = HostInfo {
            os: HostOs::Windows,
            os_name: String::new(),
            os_version: String::new(),
            arch: Arch::X86_64,
            app_arch: Arch::X86_64,
            translated: false,
            total_ram_bytes: 0,
            available_ram_bytes: 0,
            logical_cpus: 1,
            cpu_brand: String::new(),
            virtualization: Tri::Unknown,
            hypervisor_conflict: Tri::Unknown,
            hypervisor_conflict_detail: None,
            hostname_hash: String::new(),
        };
        assert!(h.is_supported_host());
        h.arch = Arch::Aarch64; // Windows on Arm: experimental in VirtualBox 7.2, not supported here
        assert!(!h.is_supported_host());
        h.os = HostOs::MacOs;
        assert!(h.is_supported_host());
        h.os = HostOs::Linux;
        assert!(!h.is_supported_host());
    }

    #[test]
    fn verbatim_prefix_is_stripped() {
        assert_eq!(
            strip_verbatim_prefix(PathBuf::from(r"\\?\C:\Users\x")),
            PathBuf::from(r"C:\Users\x")
        );
        assert_eq!(
            strip_verbatim_prefix(PathBuf::from(r"\\?\UNC\srv\share")),
            PathBuf::from(r"\\srv\share")
        );
        assert_eq!(
            strip_verbatim_prefix(PathBuf::from("/Users/x")),
            PathBuf::from("/Users/x")
        );
    }

    #[test]
    fn disk_space_on_existing_dir() {
        let d = disk_space(Path::new(".")).expect("disk");
        assert!(d.total_bytes > 0);
    }
}
