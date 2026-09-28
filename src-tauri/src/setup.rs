//! Setup orchestration on top of `vmsa-core`: inspects the host, runs downloads and the
//! installer handoff, creates the VM step by step (resumable), and derives guest status.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use vmsa_core::cmd::tokio_util_lite::CancellationToken;
use vmsa_core::download::{self, DownloadPhase, DownloadProgress, DownloadSpec, VirtualBoxRelease};
use vmsa_core::host::{self, DiskSpace, HostInfo};
use vmsa_core::installer::{self, InstallOutcome};
use vmsa_core::profile::{self, Assessment, GuestArch, ResourceLimits, StorageEstimate};
use vmsa_core::state::{DownloadRecord, GuestStatus, SetupChoices, Stage, Verified, VmRecord};
use vmsa_core::vbox::client::{self, VBoxManage};
use vmsa_core::vbox::parse::{VmInfo, VmState};
use vmsa_core::vbox::{plan, VirtualBoxInstall};
use vmsa_core::{CoreError, Result};

use crate::{AppState, APP_VERSION};

#[derive(Debug, Clone, Serialize)]
pub struct ProgressEvent {
    pub operation: String,
    pub step: String,
    pub detail: Option<String>,
    pub bytes_done: Option<u64>,
    pub bytes_total: Option<u64>,
    pub bytes_per_sec: Option<f64>,
    pub step_index: Option<usize>,
    pub step_count: Option<usize>,
}

pub fn emit(app: &AppHandle, ev: ProgressEvent) {
    let _ = app.emit("setup-progress", ev);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExistingVm {
    pub name: String,
    pub uuid: String,
    pub owned_by_this_setup: bool,
    pub created_by_app: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostReport {
    pub host: HostInfo,
    pub assessment: Assessment,
    pub virtualbox: Option<VirtualBoxInstall>,
    /// Some(false) when VirtualBox is installed but older than the profile requires.
    pub virtualbox_version_ok: Option<bool>,
    pub virtualbox_min_version: Option<String>,
    pub virtualbox_error: Option<String>,
    pub existing_vms: Vec<ExistingVm>,
    pub default_base_folder: PathBuf,
    pub disk_at_base: Option<DiskSpace>,
    pub limits: Option<ResourceLimits>,
    pub storage_estimate: Option<StorageEstimate>,
    pub guest_arch: Option<GuestArch>,
    pub downloads_dir: PathBuf,
}

pub async fn inspect_host(app: &AppState) -> Result<HostReport> {
    let host = host::inspect().await;
    let guest_arch = GuestArch::for_host(host.arch);

    let base_folder = {
        let st = app.state.lock().await;
        st.choices
            .as_ref()
            .map(|c| c.base_folder.clone())
            .unwrap_or_else(|| app.paths.default_vm_base.clone())
    };
    let iso_present = {
        let st = app.state.lock().await;
        st.choices
            .as_ref()
            .and_then(|c| c.iso_path.as_ref())
            .map(|p| p.is_file())
            .unwrap_or(false)
    };
    let disk_at_base = host::disk_space(&base_folder);
    let assessment = profile::assess(
        &host,
        disk_at_base.as_ref().map(|d| d.available_bytes),
        iso_present,
    );

    let (virtualbox, virtualbox_error) = match client::detect(host.os).await {
        Ok(v) => (v, None),
        Err(e) => (None, Some(e.to_string())),
    };
    let min_version = guest_arch.map(|g| profile::profile_for(g).min_virtualbox_version);
    let virtualbox_version_ok = match (&virtualbox, &min_version) {
        (Some(v), Some(min)) => Some(v.version.at_least(min)),
        _ => None,
    };

    let mut existing_vms = Vec::new();
    if let Some(vb) = &virtualbox {
        let vbm = VBoxManage::new(&vb.vboxmanage);
        if let Ok(list) = vbm.list_vms().await {
            let instance_id = app.state.lock().await.instance_id.clone();
            for e in list {
                let marker = vbm.owned_by_app(&e.uuid).await.unwrap_or(None);
                existing_vms.push(ExistingVm {
                    name: e.name,
                    uuid: e.uuid,
                    owned_by_this_setup: marker.as_deref() == Some(instance_id.as_str()),
                    created_by_app: marker.is_some(),
                });
            }
        }
    }

    {
        let mut st = app.state.lock().await;
        st.summarize_host(&host);
        st.virtualbox.detected_version = virtualbox.as_ref().map(|v| v.version_raw.clone());
        if virtualbox.is_some() && st.virtualbox.reboot_pending {
            st.virtualbox.reboot_pending = false;
            st.note("VirtualBox detected after restart; reboot_pending cleared");
        }
        if st.stage < Stage::CheckComputer {
            st.set_stage(Stage::CheckComputer);
        }
    }
    app.save().await?;

    Ok(HostReport {
        limits: assessment.limits.clone(),
        storage_estimate: guest_arch.map(|g| profile::storage_estimate(g, iso_present)),
        host,
        assessment,
        virtualbox,
        virtualbox_version_ok,
        virtualbox_min_version: min_version,
        virtualbox_error,
        existing_vms,
        default_base_folder: base_folder,
        disk_at_base,
        guest_arch,
        downloads_dir: app.paths.downloads_dir.clone(),
    })
}

// ---------- choices ----------

#[derive(Debug, Clone, Deserialize)]
pub struct ChoicesInput {
    pub vm_name: String,
    pub base_folder: Option<PathBuf>,
    pub ram_mb: u32,
    pub cpus: u32,
    pub disk_gb: u32,
    pub iso_path: Option<PathBuf>,
    pub iso_source: Option<vmsa_core::state::IsoSource>,
}

pub async fn set_choices(app: &AppState, input: ChoicesInput) -> Result<SetupChoices> {
    plan::validate_vm_name(&input.vm_name).map_err(CoreError::InvalidInput)?;
    let host = host::inspect().await;
    let guest_arch = GuestArch::for_host(host.arch)
        .ok_or_else(|| CoreError::Unsupported("Unknown processor architecture".into()))?;
    let limits = profile::resource_limits(&host);
    let sizing = profile::VmSizing {
        ram_mb: input.ram_mb,
        cpus: input.cpus,
        disk_gb: input.disk_gb,
    };
    let problems = profile::validate_sizing(&sizing, &limits);
    if !problems.is_empty() {
        return Err(CoreError::InvalidInput(problems.join(" ")));
    }
    let base_folder = input
        .base_folder
        .unwrap_or_else(|| app.paths.default_vm_base.clone());
    if base_folder.as_os_str().is_empty() {
        return Err(CoreError::InvalidInput(
            "Please choose a storage folder.".into(),
        ));
    }
    let mut iso_volume_id = None;
    if let Some(iso) = &input.iso_path {
        let info = vmsa_core::iso::inspect(iso)?;
        info.suitable_for(guest_arch)
            .map_err(CoreError::InvalidInput)?;
        iso_volume_id = Some(info.volume_id);
    }
    let choices = SetupChoices {
        guest_arch,
        vm_name: input.vm_name.trim().to_string(),
        base_folder,
        sizing,
        iso_path: input.iso_path,
        iso_source: input.iso_source,
        iso_volume_id,
    };
    {
        let mut st = app.state.lock().await;
        if st.vm.is_some() {
            return Err(CoreError::InvalidInput("The VM has already been created; its name, storage and size cannot be changed here.".into()));
        }
        st.choices = Some(choices.clone());
        st.clear_error();
        st.note("choices saved");
        if st.stage < Stage::ChooseSetup {
            st.set_stage(Stage::ChooseSetup);
        }
    }
    app.save().await?;
    Ok(choices)
}

// ---------- VirtualBox download / install ----------

pub async fn resolve_release(host: &HostInfo) -> Result<VirtualBoxRelease> {
    let client = download::http_client()?;
    download::resolve_virtualbox_release(&client, host.os, host.arch).await
}

pub async fn download_virtualbox(
    app: &Arc<AppState>,
    handle: &AppHandle,
) -> Result<DownloadRecord> {
    let host = host::inspect().await;
    let release = resolve_release(&host).await?;
    let dest = app.paths.downloads_dir.join(&release.file_name);
    let spec = DownloadSpec {
        display_name: format!("VirtualBox {}", release.version),
        source_label: "download.virtualbox.org".into(),
        url: release.url.clone(),
        dest: dest.clone(),
        expected_sha256: Some(release.sha256.clone()),
        expected_size: None,
    };
    let cancel = app.begin("download-virtualbox").await?;
    let client = download::http_client()?;
    let h = handle.clone();
    let name = spec.display_name.clone();
    let result = download::download(
        &client,
        &spec,
        move |p: DownloadProgress| {
            emit(
                &h,
                ProgressEvent {
                    operation: "download".into(),
                    step: match p.phase {
                        DownloadPhase::Connecting => {
                            format!("Connecting to download.virtualbox.org for {name}")
                        }
                        DownloadPhase::Downloading => format!("Downloading {name}"),
                        DownloadPhase::Verifying => format!("Verifying {name} (SHA-256)"),
                        DownloadPhase::Done => "Done".into(),
                    },
                    detail: if p.resumed_from > 0 {
                        Some("Resumed from an earlier partial download".into())
                    } else {
                        None
                    },
                    bytes_done: Some(p.bytes_done),
                    bytes_total: p.bytes_total,
                    bytes_per_sec: Some(p.bytes_per_sec),
                    step_index: None,
                    step_count: None,
                },
            );
        },
        &cancel,
    )
    .await;
    app.end().await;
    let mut st = app.state.lock().await;
    match result {
        Ok(r) => {
            let rec = DownloadRecord {
                url: release.url,
                path: r.path,
                sha256: Some(r.sha256),
                verified: r.verified,
                completed: true,
                checksum_source: Some(format!("{:?}", release.checksum_source)),
            };
            st.virtualbox.installer_download = Some(rec.clone());
            st.clear_error();
            st.note(format!(
                "VirtualBox installer downloaded and verified ({})",
                rec.path.display()
            ));
            drop(st);
            app.save().await?;
            Ok(rec)
        }
        Err(e) => {
            st.record_error(&e);
            drop(st);
            app.save().await?;
            Err(e)
        }
    }
}

/// Accept an installer the user already downloaded; verify it against the known checksum.
pub async fn use_existing_installer(app: &Arc<AppState>, path: PathBuf) -> Result<DownloadRecord> {
    let host = host::inspect().await;
    let release = resolve_release(&host).await?;
    let cancel = app.begin("verify-installer").await?;
    let sha = vmsa_core::verify::sha256_file(&path, None, Some(&cancel)).await;
    app.end().await;
    let sha = sha?;
    let verified = sha.eq_ignore_ascii_case(&release.sha256);
    if !verified {
        return Err(CoreError::ChecksumMismatch {
            file: path.display().to_string(),
            expected: release.sha256,
            actual: sha,
        });
    }
    let rec = DownloadRecord {
        url: release.url,
        path,
        sha256: Some(sha),
        verified,
        completed: true,
        checksum_source: Some(format!("{:?}", release.checksum_source)),
    };
    let mut st = app.state.lock().await;
    st.virtualbox.installer_download = Some(rec.clone());
    st.note("Existing VirtualBox installer verified");
    drop(st);
    app.save().await?;
    Ok(rec)
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallReport {
    pub outcome: InstallOutcome,
    pub detected: Option<VirtualBoxInstall>,
    pub reboot_pending: bool,
    pub message: String,
}

pub async fn run_installer(app: &Arc<AppState>, handle: &AppHandle) -> Result<InstallReport> {
    let installer = {
        let st = app.state.lock().await;
        st.virtualbox
            .installer_download
            .as_ref()
            .filter(|d| d.completed && d.verified)
            .map(|d| d.path.clone())
    }
    .ok_or_else(|| {
        CoreError::InvalidInput("Download and verify the VirtualBox installer first.".into())
    })?;
    if !installer.is_file() {
        return Err(CoreError::Io {
            path: installer.display().to_string(),
            message: "the installer file is missing; download it again".into(),
        });
    }
    let host = host::inspect().await;
    let cancel = app.begin("install-virtualbox").await?;
    let step = |s: &str, d: Option<&str>| {
        emit(
            handle,
            ProgressEvent {
                operation: "install".into(),
                step: s.into(),
                detail: d.map(String::from),
                bytes_done: None,
                bytes_total: None,
                bytes_per_sec: None,
                step_index: None,
                step_count: None,
            },
        );
    };
    let outcome: Result<InstallOutcome> = match host.os {
        host::HostOs::Windows => {
            step("Starting the VirtualBox installer", Some("Windows will ask for permission to make changes (User Account Control). Choose Yes, then follow the installer. Keep the default components."));
            installer::run_windows_installer(&installer, Some(&cancel)).await
        }
        host::HostOs::MacOs => {
            step("Opening the VirtualBox disk image", None);
            match installer::mount_dmg(&installer).await {
                Ok(m) => {
                    step("Follow the VirtualBox installer", Some("macOS will ask for your Mac login password to install. Enter it in the macOS dialog, not in this app. When the installer says the installation was successful, close it."));
                    let r = installer::open_pkg_in_installer(&m.pkg_path).await;
                    let _ = installer::unmount_dmg(&m.mount_point).await;
                    r.map(|_| InstallOutcome::Unknown)
                }
                Err(e) => Err(e),
            }
        }
        _ => Err(CoreError::Unsupported(
            "Automatic VirtualBox installation is only available on Windows and macOS.".into(),
        )),
    };
    app.end().await;
    let outcome = match outcome {
        Ok(o) => o,
        Err(e) => {
            let mut st = app.state.lock().await;
            st.record_error(&e);
            drop(st);
            app.save().await?;
            return Err(e);
        }
    };
    step("Checking whether VirtualBox is installed", None);
    let detected = client::detect(host.os).await.unwrap_or(None);
    let reboot_pending = matches!(outcome, InstallOutcome::RebootRequired);
    let message = match (&outcome, &detected) {
        (InstallOutcome::RebootRequired, _) => "VirtualBox is installed but Windows needs to restart before it works. Restart, then reopen this assistant; it will continue from here.".into(),
        (InstallOutcome::Cancelled, None) => "The installer was cancelled. Nothing was changed. You can run it again.".into(),
        (InstallOutcome::AnotherInstallRunning, _) => "Another installation is in progress on this computer. Wait for it to finish, then try again.".into(),
        (InstallOutcome::Failed { message, .. }, None) => message.clone(),
        (_, Some(v)) => format!("VirtualBox {} is installed.", v.version),
        (_, None) => "The installer finished but VirtualBox was not found. Open the installer again and complete all of its steps.".into(),
    };
    {
        let mut st = app.state.lock().await;
        st.virtualbox.installer_outcome = Some(format!("{outcome:?}"));
        st.virtualbox.reboot_pending = reboot_pending;
        st.virtualbox.detected_version = detected.as_ref().map(|v| v.version_raw.clone());
        st.note(format!(
            "installer finished: {outcome:?}; detected={}",
            detected.is_some()
        ));
        if st.stage < Stage::InstallDependencies {
            st.set_stage(Stage::InstallDependencies);
        }
    }
    app.save().await?;
    Ok(InstallReport {
        outcome,
        detected,
        reboot_pending,
        message,
    })
}

// ---------- VM creation ----------

async fn vbm_for(app: &AppState) -> Result<(VBoxManage, VirtualBoxInstall)> {
    let host = host::inspect().await;
    let vb = client::detect(host.os)
        .await?
        .ok_or(CoreError::VirtualBoxNotFound)?;
    let _ = app;
    Ok((VBoxManage::new(&vb.vboxmanage), vb))
}

pub async fn create_vm(app: &Arc<AppState>, handle: &AppHandle) -> Result<VmRecord> {
    let (choices, instance_id, existing_record) = {
        let st = app.state.lock().await;
        (st.choices.clone(), st.instance_id.clone(), st.vm.clone())
    };
    let choices =
        choices.ok_or_else(|| CoreError::InvalidInput("Choose the setup options first.".into()))?;
    let iso = choices
        .iso_path
        .clone()
        .ok_or_else(|| CoreError::InvalidInput("Select the Windows 11 ISO first.".into()))?;
    if !iso.is_file() {
        return Err(CoreError::Io {
            path: iso.display().to_string(),
            message: "the Windows ISO file is missing; select it again".into(),
        });
    }
    let (vbm, vb) = vbm_for(app).await?;
    let prof = profile::profile_for(choices.guest_arch);
    if !vb.version.at_least(&prof.min_virtualbox_version) {
        return Err(CoreError::Unsupported(format!(
            "VirtualBox {} is installed, but {} needs VirtualBox {} or newer.",
            vb.version,
            prof.guest_arch.label(),
            prof.min_virtualbox_version
        )));
    }

    // Free-space check at the moment of the expensive step.
    let est = profile::storage_estimate(choices.guest_arch, true);
    if let Some(ds) = host::disk_space(&choices.base_folder) {
        let needed = est.initial_windows_install_bytes + est.working_space_bytes;
        if ds.available_bytes < needed {
            return Err(CoreError::InsufficientDisk {
                path: choices.base_folder.display().to_string(),
                available_bytes: ds.available_bytes,
                required_bytes: needed,
            });
        }
    }

    let cancel = app.begin("create-vm").await?;
    let result = create_vm_inner(
        app,
        handle,
        &vbm,
        &vb,
        &choices,
        &instance_id,
        existing_record,
        &cancel,
    )
    .await;
    app.end().await;
    match result {
        Ok(rec) => {
            let mut st = app.state.lock().await;
            st.clear_error();
            st.set_stage(Stage::InstallWindows);
            drop(st);
            app.save().await?;
            Ok(rec)
        }
        Err(e) => {
            let mut st = app.state.lock().await;
            st.record_error(&e);
            drop(st);
            app.save().await?;
            Err(e)
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn create_vm_inner(
    app: &Arc<AppState>,
    handle: &AppHandle,
    vbm: &VBoxManage,
    vb: &VirtualBoxInstall,
    choices: &SetupChoices,
    instance_id: &str,
    existing_record: Option<VmRecord>,
    cancel: &CancellationToken,
) -> Result<VmRecord> {
    let spec = plan::CreateVmSpec {
        name: choices.vm_name.clone(),
        base_folder: choices.base_folder.clone(),
        sizing: choices.sizing.clone(),
        install_iso: choices.iso_path.clone().unwrap(),
        guest_additions_iso: vb.guest_additions_iso.clone(),
        profile: profile::profile_for(choices.guest_arch),
        instance_id: instance_id.to_string(),
        app_version: APP_VERSION.to_string(),
    };
    let disk_path = plan::disk_path_for(&spec);

    // Resume a partially created VM, or refuse to touch a VM we did not create.
    let mut record = match existing_record {
        Some(r) => {
            match vbm.vm_info(&r.uuid).await {
                Ok(_) => r,
                Err(_) => {
                    // The VM vanished (deleted in VirtualBox). Start over.
                    let mut st = app.state.lock().await;
                    st.note("recorded VM no longer exists in VirtualBox; creating again");
                    st.vm = None;
                    drop(st);
                    app.save().await?;
                    new_vm(app, handle, vbm, &spec, cancel).await?
                }
            }
        }
        None => {
            let list = vbm.list_vms().await?;
            if let Some(dup) = list.iter().find(|e| e.name == choices.vm_name) {
                let marker = vbm.owned_by_app(&dup.uuid).await?;
                if marker.as_deref() != Some(instance_id) {
                    return Err(CoreError::VmNameConflict {
                        name: dup.name.clone(),
                        uuid: dup.uuid.clone(),
                    });
                }
                // Created by this setup earlier but the state file lost it: adopt it.
                let info = vbm.vm_info(&dup.uuid).await?;
                VmRecord {
                    uuid: dup.uuid.clone(),
                    name: dup.name.clone(),
                    config_file: info.config_file.map(PathBuf::from),
                    disk_path: Some(disk_path.clone()),
                    base_folder: choices.base_folder.clone(),
                    created_by_app: true,
                    created_at: chrono::Utc::now(),
                    original_config: std::collections::BTreeMap::new(),
                    install_iso_attached: false,
                    guest_additions_iso_attached: false,
                    completed_steps: vec![],
                }
            } else {
                new_vm(app, handle, vbm, &spec, cancel).await?
            }
        }
    };

    let info = vbm.vm_info(&record.uuid).await?;
    if info.state.is_live() {
        return Err(CoreError::InvalidInput(
            "The VM is running. Shut it down before continuing setup.".into(),
        ));
    }
    let steps = plan::configure(&spec, &record.uuid, &info.storage_controllers, &disk_path);
    let total = steps.len();
    for (i, step) in steps.iter().enumerate() {
        if record.completed_steps.contains(&step.description) {
            continue;
        }
        if step.args[0] == "createmedium" && disk_path.is_file() {
            record.completed_steps.push(step.description.clone());
            continue;
        }
        emit(
            handle,
            ProgressEvent {
                operation: "create-vm".into(),
                step: step.description.clone(),
                detail: None,
                bytes_done: None,
                bytes_total: None,
                bytes_per_sec: None,
                step_index: Some(i + 1),
                step_count: Some(total),
            },
        );
        vbm.run(step, Some(cancel)).await?;
        record.completed_steps.push(step.description.clone());
        if step.args[0] == "createmedium" {
            record.disk_path = Some(disk_path.clone());
        }
        if step.args.iter().any(|a| a == "dvddrive") {
            if step
                .args
                .iter()
                .any(|a| a == &spec.install_iso.to_string_lossy())
            {
                record.install_iso_attached = true;
            } else {
                record.guest_additions_iso_attached = true;
            }
        }
        let mut st = app.state.lock().await;
        st.vm = Some(record.clone());
        drop(st);
        app.save().await?;
    }
    let info = vbm.vm_info(&record.uuid).await?;
    record.config_file = info.config_file.map(PathBuf::from);
    let mut st = app.state.lock().await;
    st.vm = Some(record.clone());
    st.note(format!("VM ready: {} ({})", record.name, record.uuid));
    drop(st);
    app.save().await?;
    Ok(record)
}

async fn new_vm(
    app: &Arc<AppState>,
    handle: &AppHandle,
    vbm: &VBoxManage,
    spec: &plan::CreateVmSpec,
    cancel: &CancellationToken,
) -> Result<VmRecord> {
    tokio::fs::create_dir_all(&spec.base_folder)
        .await
        .map_err(|e| CoreError::io(&spec.base_folder, e))?;
    let c = plan::createvm(spec);
    emit(
        handle,
        ProgressEvent {
            operation: "create-vm".into(),
            step: c.description.clone(),
            detail: None,
            bytes_done: None,
            bytes_total: None,
            bytes_per_sec: None,
            step_index: Some(0),
            step_count: None,
        },
    );
    let out = vbm.run(&c, Some(cancel)).await?;
    let (uuid, settings) =
        vmsa_core::vbox::parse::parse_createvm(&out.stdout).ok_or_else(|| {
            CoreError::Parse(format!("createvm output not understood: {}", out.stdout))
        })?;
    let record = VmRecord {
        uuid,
        name: spec.name.clone(),
        config_file: settings.map(PathBuf::from),
        disk_path: None,
        base_folder: spec.base_folder.clone(),
        created_by_app: true,
        created_at: chrono::Utc::now(),
        original_config: std::collections::BTreeMap::new(),
        install_iso_attached: false,
        guest_additions_iso_attached: false,
        completed_steps: vec![],
    };
    let mut st = app.state.lock().await;
    st.vm = Some(record.clone());
    st.note(format!("createvm succeeded: {}", record.uuid));
    drop(st);
    app.save().await?;
    Ok(record)
}

// ---------- VM status and control ----------

#[derive(Debug, Clone, Serialize)]
pub struct VmStatusReport {
    pub vm: Option<VmRecord>,
    pub info: Option<VmInfo>,
    pub guest_properties: std::collections::BTreeMap<String, String>,
    pub guest: GuestStatus,
    pub running: bool,
    pub state_label: String,
    pub virtualbox_available: bool,
}

pub async fn vm_status(app: &Arc<AppState>) -> Result<VmStatusReport> {
    let record = app.state.lock().await.vm.clone();
    let Some(record) = record else {
        let guest = app.state.lock().await.guest.clone();
        return Ok(VmStatusReport {
            vm: None,
            info: None,
            guest_properties: Default::default(),
            guest,
            running: false,
            state_label: "No VM yet".into(),
            virtualbox_available: false,
        });
    };
    let host = host::inspect().await;
    let Some(vb) = client::detect(host.os).await? else {
        let guest = app.state.lock().await.guest.clone();
        return Ok(VmStatusReport {
            vm: Some(record),
            info: None,
            guest_properties: Default::default(),
            guest,
            running: false,
            state_label: "VirtualBox not found".into(),
            virtualbox_available: false,
        });
    };
    let vbm = VBoxManage::new(&vb.vboxmanage);
    let info = vbm.vm_info(&record.uuid).await?;
    let props = if info.state == VmState::Running {
        vbm.guest_properties(&record.uuid).await.unwrap_or_default()
    } else {
        Default::default()
    };

    let mut st = app.state.lock().await;
    derive_guest_status(&mut st.guest, &info, &props);
    let guest = st.guest.clone();
    drop(st);
    app.save().await?;
    let state_label = match info.state {
        VmState::Running => "Windows is running".into(),
        VmState::Saved => "Windows is paused (state saved)".into(),
        VmState::PoweredOff => "Windows is off".into(),
        VmState::Paused => "Windows is paused".into(),
        VmState::Aborted => "Windows stopped unexpectedly".into(),
        VmState::Starting => "Starting".into(),
        VmState::Stopping => "Stopping".into(),
        VmState::Saving => "Saving".into(),
        VmState::Restoring => "Resuming".into(),
        VmState::Other => format!("VirtualBox state: {}", info.state_raw),
    };
    Ok(VmStatusReport {
        running: info.state == VmState::Running,
        vm: Some(record),
        info: Some(info),
        guest_properties: props,
        guest,
        state_label,
        virtualbox_available: true,
    })
}

/// Update the observable parts of the guest status. User-confirmed items are left alone.
pub fn derive_guest_status(
    g: &mut GuestStatus,
    info: &VmInfo,
    props: &std::collections::BTreeMap<String, String>,
) {
    let running = info.state == VmState::Running;
    g.windows_running = if running { Verified::Yes } else { Verified::No };
    if running {
        let ga_version = props
            .get("/VirtualBox/GuestAdd/Version")
            .cloned()
            .or_else(|| info.guest_additions_version.clone());
        let ga_active = info
            .guest_additions_run_level
            .map(|l| l >= 2)
            .unwrap_or(false)
            || ga_version.is_some();
        if ga_active {
            g.guest_additions = Verified::Yes;
            g.guest_additions_version = ga_version;
            g.evidence.insert(
                "guest_additions".into(),
                "Guest Additions reported their version to VirtualBox".into(),
            );
            if g.windows_installed != Verified::Yes {
                g.windows_installed = Verified::Yes;
                g.evidence.insert(
                    "windows_installed".into(),
                    "Guest Additions are running, which requires an installed Windows".into(),
                );
            }
        } else {
            g.guest_additions = Verified::No;
        }
        match props
            .get("/VirtualBox/GuestInfo/Net/0/Status")
            .map(|s| s.as_str())
        {
            Some("Up") => {
                g.network_link = Verified::Yes;
                g.evidence.insert(
                    "network_link".into(),
                    format!(
                        "Adapter reports Up, address {}",
                        props
                            .get("/VirtualBox/GuestInfo/Net/0/V4/IP")
                            .cloned()
                            .unwrap_or_else(|| "none".into())
                    ),
                );
            }
            Some(_) => g.network_link = Verified::No,
            None => {
                if g.guest_additions != Verified::Yes {
                    g.network_link = Verified::Pending;
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GuestItem {
    WindowsInstalled,
    InternetAccess,
    WindowsUpdatesComplete,
    WindowsActivated,
    GuestAdditions,
}

pub async fn confirm_guest_item(
    app: &Arc<AppState>,
    item: GuestItem,
    yes: bool,
) -> Result<GuestStatus> {
    let mut st = app.state.lock().await;
    let v = if yes { Verified::Yes } else { Verified::No };
    let key = match item {
        GuestItem::WindowsInstalled => {
            st.guest.windows_installed = v;
            "windows_installed"
        }
        GuestItem::InternetAccess => {
            st.guest.internet_access = v;
            "internet_access"
        }
        GuestItem::WindowsUpdatesComplete => {
            st.guest.windows_updates_complete = v;
            "windows_updates_complete"
        }
        GuestItem::WindowsActivated => {
            st.guest.windows_activated = v;
            "windows_activated"
        }
        GuestItem::GuestAdditions => {
            st.guest.guest_additions = v;
            "guest_additions"
        }
    };
    st.guest.evidence.insert(
        key.into(),
        format!(
            "User confirmed {} on {}",
            if yes { "yes" } else { "no" },
            chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
        ),
    );
    st.note(format!("user confirmed {key}={yes}"));
    if item == GuestItem::WindowsInstalled && yes && st.stage < Stage::FinishAndVerify {
        st.set_stage(Stage::FinishAndVerify);
    }
    let g = st.guest.clone();
    drop(st);
    app.save().await?;
    Ok(g)
}

pub async fn control(app: &Arc<AppState>, action: &str) -> Result<String> {
    let record = app
        .state
        .lock()
        .await
        .vm
        .clone()
        .ok_or_else(|| CoreError::InvalidInput("No VM has been created yet.".into()))?;
    let (vbm, _) = vbm_for(app).await?;
    let info = vbm.vm_info(&record.uuid).await?;
    let (c, msg) = match action {
        "start" => {
            if info.state.is_live() {
                return Ok(
                    "Windows is already running. Its window may be behind other windows.".into(),
                );
            }
            (
                plan::start(&record.uuid),
                "Starting Windows in its own window.",
            )
        }
        "shutdown" => {
            if !info.state.is_live() {
                return Ok("Windows is not running.".into());
            }
            (plan::acpi_shutdown(&record.uuid), "Asked Windows to shut down. If Windows shows a dialog, answer it inside the Windows window.")
        }
        "savestate" => {
            if !info.state.is_live() {
                return Ok("Windows is not running.".into());
            }
            (
                plan::save_state(&record.uuid),
                "Saving Windows' state. Next start resumes exactly where you left off.",
            )
        }
        "poweroff" => {
            if !info.state.is_live() {
                return Ok("Windows is not running.".into());
            }
            (
                plan::power_off(&record.uuid),
                "Powered off. Windows may check its disk on the next start.",
            )
        }
        _ => return Err(CoreError::InvalidInput(format!("unknown action {action}"))),
    };
    let _ = app.begin("vm-control").await?;
    let r = vbm.run(&c, None).await;
    app.end().await;
    r?;
    let mut st = app.state.lock().await;
    st.note(format!("vm control: {action}"));
    if action == "start" && st.stage < Stage::InstallWindows {
        st.set_stage(Stage::InstallWindows);
    }
    drop(st);
    app.save().await?;
    Ok(msg.into())
}

pub async fn eject_install_iso(app: &Arc<AppState>) -> Result<String> {
    let record = app
        .state
        .lock()
        .await
        .vm
        .clone()
        .ok_or_else(|| CoreError::InvalidInput("No VM has been created yet.".into()))?;
    let (vbm, _) = vbm_for(app).await?;
    let info = vbm.vm_info(&record.uuid).await?;
    let choices = app.state.lock().await.choices.clone();
    let iso = choices
        .and_then(|c| c.iso_path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let Some(att) = info.attachments.iter().find(|a| a.medium == iso) else {
        return Ok("The Windows installation disc is not inserted.".into());
    };
    let c = plan::eject_dvd(
        &record.uuid,
        &att.controller,
        att.port,
        info.state.is_live(),
    );
    vbm.run(&c, None).await?;
    let mut st = app.state.lock().await;
    if let Some(v) = st.vm.as_mut() {
        v.install_iso_attached = false;
    }
    st.note("install ISO ejected");
    drop(st);
    app.save().await?;
    Ok("The Windows installation disc was ejected. Windows will now always start from its own disk.".into())
}

// ---------- network ----------

pub async fn network_facts(
    app: &Arc<AppState>,
    user_error: Option<String>,
) -> Result<(vmsa_core::network::NetworkFacts, GuestArch)> {
    let (record, guest_arch, internet) = {
        let st = app.state.lock().await;
        (
            st.vm.clone(),
            st.choices.as_ref().map(|c| c.guest_arch),
            st.guest.internet_access,
        )
    };
    let record =
        record.ok_or_else(|| CoreError::InvalidInput("No VM has been created yet.".into()))?;
    let guest_arch = guest_arch.unwrap_or(GuestArch::X64);
    let (vbm, _) = vbm_for(app).await?;
    let info = vbm.vm_info(&record.uuid).await?;
    let running = info.state == VmState::Running;
    let props = if running {
        vbm.guest_properties(&record.uuid).await.unwrap_or_default()
    } else {
        Default::default()
    };
    let ga_active = running
        && (info
            .guest_additions_run_level
            .map(|l| l >= 2)
            .unwrap_or(false)
            || props.contains_key("/VirtualBox/GuestAdd/Version"));
    let facts = vmsa_core::network::NetworkFacts {
        vm_running: running,
        guest_additions_active: ga_active,
        nic_attachment: info.nic1.clone(),
        nic_type: info.nic1_type.clone(),
        cable_connected: info.nic1_cable_connected,
        guest_adapter_count: props
            .get("/VirtualBox/GuestInfo/Net/Count")
            .and_then(|v| v.parse().ok()),
        guest_adapter_status: props.get("/VirtualBox/GuestInfo/Net/0/Status").cloned(),
        guest_ipv4: props.get("/VirtualBox/GuestInfo/Net/0/V4/IP").cloned(),
        user_confirmed_internet: match internet {
            Verified::Yes => Some(true),
            Verified::No => Some(false),
            Verified::Pending => None,
        },
        user_reported_device_error: user_error,
    };
    Ok((facts, guest_arch))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RepairRequest {
    AttachNat,
    ConnectCable,
    ReconnectCable,
    SwitchNicType { to: String },
    RestoreNicType,
}

pub async fn apply_repair(app: &Arc<AppState>, req: RepairRequest) -> Result<String> {
    let record = app
        .state
        .lock()
        .await
        .vm
        .clone()
        .ok_or_else(|| CoreError::InvalidInput("No VM has been created yet.".into()))?;
    let (vbm, _) = vbm_for(app).await?;
    let info = vbm.vm_info(&record.uuid).await?;
    let live = info.state.is_live();
    let _ = app.begin("network-repair").await?;
    let r: Result<String> = async {
        match req {
            RepairRequest::AttachNat => {
                if live {
                    return Err(CoreError::InvalidInput("Shut Windows down first, then attach the network adapter.".into()));
                }
                vbm.run_raw(&["modifyvm".into(), record.uuid.clone(), "--nic1".into(), "nat".into(), "--cable-connected1".into(), "on".into()], std::time::Duration::from_secs(60), None).await?;
                Ok("The VM now has a NAT network adapter.".into())
            }
            RepairRequest::ConnectCable => {
                vbm.run(&plan::set_cable(&record.uuid, true, live), None).await?;
                Ok("The virtual network cable is connected.".into())
            }
            RepairRequest::ReconnectCable => {
                vbm.run(&plan::set_cable(&record.uuid, false, live), None).await?;
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                vbm.run(&plan::set_cable(&record.uuid, true, live), None).await?;
                Ok("The virtual network cable was unplugged and plugged back in. Give Windows about 30 seconds to reconnect.".into())
            }
            RepairRequest::SwitchNicType { to } => {
                if live {
                    return Err(CoreError::InvalidInput("Shut Windows down first; the adapter type can only be changed while the VM is off.".into()));
                }
                let allowed = ["virtio", "usbnet", "82540EM", "82543GC", "82545EM", "Am79C973"];
                if !allowed.contains(&to.as_str()) {
                    return Err(CoreError::InvalidInput(format!("Unknown adapter type {to}")));
                }
                // Record the original type once so it can be restored.
                let mut st = app.state.lock().await;
                if let Some(v) = st.vm.as_mut() {
                    if !v.original_config.contains_key("nictype1") {
                        v.original_config.insert("nictype1".into(), info.nic1_type.clone().unwrap_or_default());
                    }
                }
                drop(st);
                app.save().await?;
                vbm.run(&plan::set_nic_type(&record.uuid, &to), None).await?;
                Ok(format!("The virtual network adapter is now '{to}'. The previous setting was saved and can be restored from Troubleshooting."))
            }
            RepairRequest::RestoreNicType => {
                if live {
                    return Err(CoreError::InvalidInput("Shut Windows down first.".into()));
                }
                let orig = app.state.lock().await.vm.as_ref().and_then(|v| v.original_config.get("nictype1").cloned());
                let Some(orig) = orig.filter(|s| !s.is_empty()) else { return Ok("There is no saved adapter setting to restore.".into()) };
                vbm.run(&plan::set_nic_type(&record.uuid, &orig), None).await?;
                Ok(format!("Restored the original adapter type '{orig}'."))
            }
        }
    }
    .await;
    app.end().await;
    let msg = r?;
    let mut st = app.state.lock().await;
    st.note(format!("network repair: {msg}"));
    drop(st);
    app.save().await?;
    Ok(msg)
}

// ---------- support report ----------

pub async fn support_report(app: &Arc<AppState>) -> Result<String> {
    use vmsa_core::diagnostics::{build_report, ReportSection};
    let mut sections = Vec::new();
    let host = host::inspect().await;
    sections.push(ReportSection {
        title: "Host".into(),
        body: serde_json::to_string_pretty(&host).unwrap_or_default(),
    });
    let st = app.state.lock().await.clone();
    let mut st_json = serde_json::to_value(&st).unwrap_or_default();
    if let Some(obj) = st_json.as_object_mut() {
        obj.remove("history");
    }
    sections.push(ReportSection {
        title: "Setup state".into(),
        body: serde_json::to_string_pretty(&st_json).unwrap_or_default(),
    });
    let hist: Vec<String> = st
        .history
        .iter()
        .rev()
        .take(60)
        .map(|e| {
            format!(
                "{} [{:?}] {}",
                e.at.format("%Y-%m-%d %H:%M:%S"),
                e.stage,
                e.message
            )
        })
        .collect();
    sections.push(ReportSection {
        title: "Recent events".into(),
        body: hist.join("\n"),
    });
    if let Ok(Some(vb)) = client::detect(host.os).await {
        let vbm = VBoxManage::new(&vb.vboxmanage);
        sections.push(ReportSection {
            title: "VirtualBox".into(),
            body: format!("{} at {}", vb.version_raw, vb.vboxmanage.display()),
        });
        if let Some(vm) = &st.vm {
            if let Ok(out) = vbm.run(&plan::showvminfo(&vm.uuid), None).await {
                sections.push(ReportSection {
                    title: "VM settings (showvminfo)".into(),
                    body: out.stdout,
                });
            }
            if let Ok(out) = vbm.run(&plan::guest_properties(&vm.uuid), None).await {
                sections.push(ReportSection {
                    title: "Guest properties".into(),
                    body: out.stdout,
                });
            }
            if let Ok(info) = vbm.vm_info(&vm.uuid).await {
                if let Some(log_dir) = info.log_folder {
                    let log = Path::new(&log_dir).join("VBox.log");
                    if let Ok(text) = tokio::fs::read_to_string(&log).await {
                        let tail: Vec<&str> = text
                            .lines()
                            .rev()
                            .take(300)
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev()
                            .collect();
                        sections.push(ReportSection {
                            title: "VBox.log (last 300 lines)".into(),
                            body: tail.join("\n"),
                        });
                    }
                }
            }
        }
    }
    let log_path = app.paths.logs_dir.join("vm-setup-assistant.log");
    if let Ok(text) = tokio::fs::read_to_string(&log_path).await {
        let tail: Vec<&str> = text
            .lines()
            .rev()
            .take(200)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        sections.push(ReportSection {
            title: "Assistant log (last 200 lines)".into(),
            body: tail.join("\n"),
        });
    }
    Ok(build_report(APP_VERSION, &sections))
}

// ---------- destructive ----------

/// Delete the VM and its files. Only allowed for a VM this app created, and the caller must pass
/// the VM's exact name as confirmation.
pub async fn delete_vm(app: &Arc<AppState>, confirm_name: &str) -> Result<String> {
    let record = app
        .state
        .lock()
        .await
        .vm
        .clone()
        .ok_or_else(|| CoreError::InvalidInput("No VM has been created yet.".into()))?;
    if !record.created_by_app {
        return Err(CoreError::InvalidInput("This VM was not created by VM Setup Assistant, so it will not delete it. Use VirtualBox to manage it.".into()));
    }
    if confirm_name != record.name {
        return Err(CoreError::InvalidInput(
            "The name you typed does not match the VM name.".into(),
        ));
    }
    let (vbm, _) = vbm_for(app).await?;
    let info = vbm.vm_info(&record.uuid).await?;
    if info.state.is_live() {
        return Err(CoreError::InvalidInput(
            "Shut Windows down before deleting the VM.".into(),
        ));
    }
    let marker = vbm.owned_by_app(&record.uuid).await?;
    if marker.is_none() {
        return Err(CoreError::InvalidInput(
            "VirtualBox no longer marks this VM as created by this app; refusing to delete it."
                .into(),
        ));
    }
    let _ = app.begin("delete-vm").await?;
    let r = vbm
        .run(&plan::unregister_and_delete(&record.uuid), None)
        .await;
    app.end().await;
    r?;
    let mut st = app.state.lock().await;
    st.vm = None;
    st.guest = GuestStatus::default();
    st.set_stage(Stage::ChooseSetup);
    st.note(format!("VM {} deleted by user request", record.name));
    drop(st);
    app.save().await?;
    Ok("The VM and its virtual disk were deleted.".into())
}
