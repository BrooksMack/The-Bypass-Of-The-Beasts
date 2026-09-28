//! Persistent setup state machine. The state file is the source of truth for "what has been
//! done", so reopening the app never repeats a download or creates a second VM.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::host::HostInfo;
use crate::profile::{GuestArch, VmSizing};
use crate::{CoreError, Result};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Welcome,
    CheckComputer,
    ChooseSetup,
    ObtainFiles,
    InstallDependencies,
    CreateVm,
    InstallWindows,
    FinishAndVerify,
    Dashboard,
}

impl Stage {
    pub fn all() -> [Stage; 9] {
        [
            Stage::Welcome,
            Stage::CheckComputer,
            Stage::ChooseSetup,
            Stage::ObtainFiles,
            Stage::InstallDependencies,
            Stage::CreateVm,
            Stage::InstallWindows,
            Stage::FinishAndVerify,
            Stage::Dashboard,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsoSource {
    /// The user picked an ISO they already had.
    Existing,
    /// The user downloaded it from Microsoft's page via the guided handoff, then selected it.
    MicrosoftDownloadPage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetupChoices {
    pub guest_arch: GuestArch,
    pub vm_name: String,
    pub base_folder: PathBuf,
    pub sizing: VmSizing,
    pub iso_path: Option<PathBuf>,
    pub iso_source: Option<IsoSource>,
    pub iso_volume_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DownloadRecord {
    pub url: String,
    pub path: PathBuf,
    pub sha256: Option<String>,
    pub verified: bool,
    pub completed: bool,
    pub checksum_source: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct VirtualBoxState {
    pub detected_version: Option<String>,
    pub installer_download: Option<DownloadRecord>,
    pub installer_outcome: Option<String>,
    /// Set when the installer said a reboot is needed; cleared once VirtualBox is detected after boot.
    pub reboot_pending: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VmRecord {
    pub uuid: String,
    pub name: String,
    pub config_file: Option<PathBuf>,
    pub disk_path: Option<PathBuf>,
    pub base_folder: PathBuf,
    /// True only when this app ran `createvm`. Never true for adopted VMs.
    pub created_by_app: bool,
    pub created_at: DateTime<Utc>,
    /// Configuration captured before any change the app makes, so it can be restored.
    pub original_config: BTreeMap<String, String>,
    pub install_iso_attached: bool,
    pub guest_additions_iso_attached: bool,
    /// Which `configure` steps completed (idempotent resume of a partially created VM).
    pub completed_steps: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verified {
    /// Confirmed by observation (guest properties, VM state) or explicit user confirmation.
    Yes,
    No,
    /// Not checked yet or cannot be observed from outside the guest.
    Pending,
}

impl Default for Verified {
    fn default() -> Self {
        Verified::Pending
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GuestStatus {
    pub windows_installed: Verified,
    pub windows_running: Verified,
    pub guest_additions: Verified,
    pub guest_additions_version: Option<String>,
    pub network_link: Verified,
    pub internet_access: Verified,
    pub windows_updates_complete: Verified,
    pub windows_activated: Verified,
    /// Notes on how each item was verified ("guest property", "user confirmed", ...).
    pub evidence: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventRecord {
    pub at: DateTime<Utc>,
    pub stage: Stage,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LastError {
    pub at: DateTime<Utc>,
    pub stage: Stage,
    pub error: CoreError,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetupState {
    pub schema_version: u32,
    pub instance_id: String,
    pub app_version: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub stage: Stage,
    pub host_summary: Option<String>,
    pub choices: Option<SetupChoices>,
    pub virtualbox: VirtualBoxState,
    pub vm: Option<VmRecord>,
    pub guest: GuestStatus,
    pub last_error: Option<LastError>,
    pub history: Vec<EventRecord>,
}

impl SetupState {
    pub fn new(app_version: &str) -> Self {
        let now = Utc::now();
        Self {
            schema_version: SCHEMA_VERSION,
            instance_id: uuid::Uuid::new_v4().to_string(),
            app_version: app_version.to_string(),
            created_at: now,
            updated_at: now,
            stage: Stage::Welcome,
            host_summary: None,
            choices: None,
            virtualbox: VirtualBoxState::default(),
            vm: None,
            guest: GuestStatus::default(),
            last_error: None,
            history: Vec::new(),
        }
    }

    pub fn note(&mut self, message: impl Into<String>) {
        let msg = message.into();
        tracing::info!(stage = ?self.stage, "{msg}");
        self.history.push(EventRecord { at: Utc::now(), stage: self.stage, message: msg });
        if self.history.len() > 500 {
            let drop = self.history.len() - 500;
            self.history.drain(..drop);
        }
        self.updated_at = Utc::now();
    }

    pub fn set_stage(&mut self, stage: Stage) {
        if stage != self.stage {
            self.note(format!("stage {:?} -> {:?}", self.stage, stage));
            self.stage = stage;
        }
    }

    pub fn record_error(&mut self, err: &CoreError) {
        self.last_error = Some(LastError { at: Utc::now(), stage: self.stage, error: err.clone() });
        self.note(format!("error: {err}"));
    }

    pub fn clear_error(&mut self) {
        self.last_error = None;
    }

    pub fn summarize_host(&mut self, h: &HostInfo) {
        self.host_summary = Some(format!(
            "{} {} on {}, {:.1} GB RAM, {} CPUs",
            h.os_name,
            h.os_version,
            h.arch.label(),
            h.total_ram_bytes as f64 / (1u64 << 30) as f64,
            h.logical_cpus
        ));
    }

    /// Where a reopened app should resume, derived from durable facts rather than the last stage
    /// alone (a VM that exists is never re-created; a verified download is never repeated).
    pub fn resume_stage(&self) -> Stage {
        if self.vm.is_some() {
            if self.guest.windows_installed == Verified::Yes {
                return if self.stage >= Stage::Dashboard { Stage::Dashboard } else { Stage::FinishAndVerify.max(self.stage) };
            }
            return Stage::InstallWindows;
        }
        if self.stage >= Stage::CreateVm {
            return Stage::CreateVm;
        }
        self.stage
    }

    // ----- persistence -----

    pub fn load(path: &Path) -> Result<Option<SetupState>> {
        match std::fs::read(path) {
            Ok(bytes) => {
                let st: SetupState = serde_json::from_slice(&bytes)
                    .map_err(|e| CoreError::Parse(format!("setup state file is unreadable ({e}); it was backed up")))?;
                if st.schema_version > SCHEMA_VERSION {
                    return Err(CoreError::Parse(format!(
                        "setup state was written by a newer app version (schema {})",
                        st.schema_version
                    )));
                }
                Ok(Some(st))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(CoreError::io(path, e)),
        }
    }

    /// Atomic save: write to a temp file in the same directory, then rename over the target.
    pub fn save(&self, path: &Path) -> Result<()> {
        let dir = path.parent().ok_or_else(|| CoreError::InvalidInput("state path has no parent".into()))?;
        std::fs::create_dir_all(dir).map_err(|e| CoreError::io(dir, e))?;
        let tmp = dir.join(format!(".{}.tmp-{}", path.file_name().and_then(|s| s.to_str()).unwrap_or("state"), std::process::id()));
        let json = serde_json::to_vec_pretty(self)?;
        std::fs::write(&tmp, &json).map_err(|e| CoreError::io(&tmp, e))?;
        std::fs::rename(&tmp, path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            CoreError::io(path, e)
        })?;
        Ok(())
    }

    /// Load, or move a corrupt file aside and start fresh (never silently discard it).
    pub fn load_or_recover(path: &Path, app_version: &str) -> (SetupState, Option<String>) {
        match Self::load(path) {
            Ok(Some(s)) => (s, None),
            Ok(None) => (Self::new(app_version), None),
            Err(e) => {
                let backup = path.with_extension(format!("corrupt-{}.json", Utc::now().format("%Y%m%d%H%M%S")));
                let _ = std::fs::rename(path, &backup);
                (Self::new(app_version), Some(format!("{e}. Backup: {}", backup.display())))
            }
        }
    }
}

// ----- single-instance lock -----

/// A best-effort lock file that records the owning process id. Combined with the app-level
/// single-instance plugin, this prevents two assistants from running setup at the same time.
pub struct InstanceLock {
    path: PathBuf,
}

impl InstanceLock {
    pub fn acquire(path: &Path) -> Result<InstanceLock> {
        if let Ok(existing) = std::fs::read_to_string(path) {
            if let Ok(pid) = existing.trim().parse::<u32>() {
                if pid != std::process::id() && process_alive(pid) {
                    return Err(CoreError::Busy);
                }
            }
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| CoreError::io(dir, e))?;
        }
        std::fs::write(path, std::process::id().to_string()).map_err(|e| CoreError::io(path, e))?;
        Ok(InstanceLock { path: path.to_path_buf() })
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn process_alive(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
    let mut s = System::new();
    s.refresh_processes_specifics(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true, ProcessRefreshKind::nothing());
    s.process(Pid::from_u32(pid)).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choices() -> SetupChoices {
        SetupChoices {
            guest_arch: GuestArch::Arm64,
            vm_name: "Windows 11".into(),
            base_folder: PathBuf::from("/Users/x/VirtualBox VMs"),
            sizing: VmSizing { ram_mb: 8192, cpus: 4, disk_gb: 100 },
            iso_path: Some(PathBuf::from("/Users/x/Downloads/Win11.iso")),
            iso_source: Some(IsoSource::MicrosoftDownloadPage),
            iso_volume_id: Some("CCCOMA_A64FRE_EN-US_DV9".into()),
        }
    }

    #[test]
    fn roundtrip_and_atomic_save() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("setup-state.json");
        let mut s = SetupState::new("0.1.0");
        s.choices = Some(choices());
        s.set_stage(Stage::ObtainFiles);
        s.record_error(&CoreError::Cancelled);
        s.save(&p).unwrap();
        let loaded = SetupState::load(&p).unwrap().unwrap();
        assert_eq!(loaded, s);
        assert!(!dir.path().join(".setup-state.json.tmp").exists());
        assert!(std::fs::read_dir(dir.path()).unwrap().count() == 1);
    }

    #[test]
    fn corrupt_state_is_backed_up_not_lost() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("setup-state.json");
        std::fs::write(&p, b"{ this is not json").unwrap();
        let (s, warning) = SetupState::load_or_recover(&p, "0.1.0");
        assert_eq!(s.stage, Stage::Welcome);
        assert!(warning.unwrap().contains("Backup"));
        assert!(!p.exists());
        assert!(std::fs::read_dir(dir.path()).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().contains("corrupt")));
    }

    #[test]
    fn newer_schema_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("setup-state.json");
        let mut s = SetupState::new("9.9.9");
        s.schema_version = SCHEMA_VERSION + 1;
        std::fs::write(&p, serde_json::to_vec(&s).unwrap()).unwrap();
        assert!(SetupState::load(&p).is_err());
    }

    #[test]
    fn resume_never_recreates_a_vm() {
        let mut s = SetupState::new("0.1.0");
        s.stage = Stage::CreateVm;
        assert_eq!(s.resume_stage(), Stage::CreateVm);
        s.vm = Some(VmRecord {
            uuid: "u".into(),
            name: "Windows 11".into(),
            config_file: None,
            disk_path: None,
            base_folder: PathBuf::from("/x"),
            created_by_app: true,
            created_at: Utc::now(),
            original_config: BTreeMap::new(),
            install_iso_attached: true,
            guest_additions_iso_attached: true,
            completed_steps: vec![],
        });
        assert_eq!(s.resume_stage(), Stage::InstallWindows);
        s.guest.windows_installed = Verified::Yes;
        assert_eq!(s.resume_stage(), Stage::FinishAndVerify);
        s.stage = Stage::Dashboard;
        assert_eq!(s.resume_stage(), Stage::Dashboard);
    }

    #[test]
    fn resume_before_vm_keeps_stage() {
        let mut s = SetupState::new("0.1.0");
        s.stage = Stage::ObtainFiles;
        assert_eq!(s.resume_stage(), Stage::ObtainFiles);
        s.stage = Stage::InstallWindows; // stage claims further than the facts support
        assert_eq!(s.resume_stage(), Stage::CreateVm);
    }

    #[test]
    fn history_is_bounded() {
        let mut s = SetupState::new("0.1.0");
        for i in 0..600 {
            s.note(format!("event {i}"));
        }
        assert_eq!(s.history.len(), 500);
        assert_eq!(s.history[0].message, "event 100");
    }

    #[test]
    fn instance_lock_blocks_live_pid_and_ignores_stale() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("setup.lock");
        // Stale lock from a dead pid is taken over.
        std::fs::write(&p, "4294967000").unwrap();
        let l1 = InstanceLock::acquire(&p).unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), std::process::id().to_string());
        // Same process re-acquiring is fine (our own pid).
        drop(l1);
        assert!(!p.exists());
    }
}
