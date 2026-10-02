//! Thin Tauri commands. Each one validates input, delegates to `setup`, and returns
//! serializable results. The web view never gets a general-purpose shell or file API.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};

use vmsa_core::download::VirtualBoxRelease;
use vmsa_core::iso::IsoInfo;
use vmsa_core::state::{DownloadRecord, GuestStatus, SetupChoices, SetupState, Stage};
use vmsa_core::{CoreError, Result};

use crate::setup::{
    self, ChoicesInput, GuestItem, HostReport, InstallReport, RepairRequest, VmStatusReport,
};
use crate::{AppState, APP_VERSION};

type App<'a> = State<'a, Arc<AppState>>;

#[derive(Serialize)]
pub struct AppInfo {
    pub version: String,
    pub app_arch: vmsa_core::host::Arch,
    pub data_dir: PathBuf,
    pub downloads_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub startup_warning: Option<String>,
    pub busy_with: Option<String>,
}

#[tauri::command]
pub async fn get_app_info(app: App<'_>) -> Result<AppInfo> {
    Ok(AppInfo {
        version: APP_VERSION.into(),
        app_arch: vmsa_core::host::APP_ARCH,
        data_dir: app.paths.data_dir.clone(),
        downloads_dir: app.paths.downloads_dir.clone(),
        logs_dir: app.paths.logs_dir.clone(),
        startup_warning: app.startup_warning.lock().await.clone(),
        busy_with: app.operation.lock().await.as_ref().map(|o| o.name.clone()),
    })
}

#[tauri::command]
pub async fn acknowledge_startup_warning(app: App<'_>) -> Result<()> {
    *app.startup_warning.lock().await = None;
    Ok(())
}

#[tauri::command]
pub async fn get_state(app: App<'_>) -> Result<SetupState> {
    Ok(app.state.lock().await.clone())
}

#[tauri::command]
pub async fn set_stage(app: App<'_>, stage: Stage) -> Result<SetupState> {
    let mut st = app.state.lock().await;
    st.set_stage(stage);
    let s = st.clone();
    drop(st);
    app.save().await?;
    Ok(s)
}

#[tauri::command]
pub async fn inspect_host(app: App<'_>) -> Result<HostReport> {
    setup::inspect_host(&app).await
}

#[tauri::command]
pub async fn inspect_iso(path: PathBuf) -> Result<IsoInfo> {
    vmsa_core::iso::inspect(&path)
}

#[tauri::command]
pub async fn set_choices(app: App<'_>, input: ChoicesInput) -> Result<SetupChoices> {
    setup::set_choices(&app, input).await
}

#[tauri::command]
pub async fn resolve_virtualbox_release() -> Result<VirtualBoxRelease> {
    let host = vmsa_core::host::inspect().await;
    setup::resolve_release(&host).await
}

#[tauri::command]
pub async fn download_virtualbox(app: App<'_>, handle: AppHandle) -> Result<DownloadRecord> {
    setup::download_virtualbox(&app, &handle).await
}

#[tauri::command]
pub async fn use_existing_virtualbox_installer(
    app: App<'_>,
    path: PathBuf,
) -> Result<DownloadRecord> {
    setup::use_existing_installer(&app, path).await
}

#[tauri::command]
pub async fn run_virtualbox_installer(app: App<'_>, handle: AppHandle) -> Result<InstallReport> {
    setup::run_installer(&app, &handle).await
}

#[tauri::command]
pub async fn redetect_virtualbox(
    app: App<'_>,
) -> Result<Option<vmsa_core::vbox::VirtualBoxInstall>> {
    let host = vmsa_core::host::inspect().await;
    let v = vmsa_core::vbox::client::detect(host.os).await?;
    let mut st = app.state.lock().await;
    st.virtualbox.detected_version = v.as_ref().map(|x| x.version_raw.clone());
    st.virtualbox
        .observe_boot(vmsa_core::host::boot_time(), v.is_some());
    drop(st);
    app.save().await?;
    Ok(v)
}

#[tauri::command]
pub async fn cancel_operation(app: App<'_>) -> Result<bool> {
    let op = app.operation.lock().await;
    match op.as_ref() {
        Some(o) => {
            o.cancel.cancel();
            Ok(true)
        }
        None => Ok(false),
    }
}

#[tauri::command]
pub async fn create_vm(app: App<'_>, handle: AppHandle) -> Result<vmsa_core::state::VmRecord> {
    setup::create_vm(&app, &handle).await
}

#[tauri::command]
pub async fn vm_status(app: App<'_>) -> Result<VmStatusReport> {
    setup::vm_status(&app).await
}

#[tauri::command]
pub async fn start_vm(app: App<'_>) -> Result<String> {
    setup::control(&app, "start").await
}

#[tauri::command]
pub async fn shutdown_vm(app: App<'_>) -> Result<String> {
    setup::control(&app, "shutdown").await
}

#[tauri::command]
pub async fn save_state_vm(app: App<'_>) -> Result<String> {
    setup::control(&app, "savestate").await
}

#[tauri::command]
pub async fn power_off_vm(app: App<'_>) -> Result<String> {
    setup::control(&app, "poweroff").await
}

#[tauri::command]
pub async fn confirm_guest_item(app: App<'_>, item: GuestItem, yes: bool) -> Result<GuestStatus> {
    setup::confirm_guest_item(&app, item, yes).await
}

#[tauri::command]
pub async fn eject_install_iso(app: App<'_>) -> Result<String> {
    setup::eject_install_iso(&app).await
}

#[tauri::command]
pub async fn network_diagnose(
    app: App<'_>,
    user_error: Option<String>,
) -> Result<vmsa_core::network::Diagnosis> {
    let (facts, arch) = setup::network_facts(&app, user_error).await?;
    Ok(vmsa_core::network::diagnose(&facts, arch))
}

#[tauri::command]
pub async fn network_apply_repair(app: App<'_>, request: RepairRequest) -> Result<String> {
    setup::apply_repair(&app, request).await
}

#[tauri::command]
pub async fn build_support_report(app: App<'_>) -> Result<String> {
    setup::support_report(&app).await
}

#[tauri::command]
pub async fn save_support_report(path: PathBuf, text: String) -> Result<()> {
    // The path comes from the OS save dialog. Re-redact defensively before writing.
    let text = vmsa_core::diagnostics::redact(&text);
    tokio::fs::write(&path, text)
        .await
        .map_err(|e| CoreError::io(&path, e))
}

/// Only fixed official pages can be opened from the UI; arbitrary URLs are refused.
#[tauri::command]
pub async fn open_official_page(page: String) -> Result<()> {
    let troubleshooting = format!("https://github.com/BrooksMack/The-Bypass-Of-The-Beasts/blob/v{APP_VERSION}/docs/TROUBLESHOOTING.md");
    let url = match page.as_str() {
        "windows11_x64" => "https://www.microsoft.com/software-download/windows11",
        "windows11_arm64" => "https://www.microsoft.com/software-download/windows11arm64",
        "virtualbox_downloads" => "https://www.virtualbox.org/wiki/Downloads",
        "virtualbox_docs" => "https://docs.oracle.com/en/virtualization/virtualbox/7.2/",
        "virtio_win" => "https://github.com/virtio-win/virtio-win-pkg-scripts",
        "project" => "https://github.com/BrooksMack/The-Bypass-Of-The-Beasts",
        "project_issues" => "https://github.com/BrooksMack/The-Bypass-Of-The-Beasts/issues",
        "project_troubleshooting" => troubleshooting.as_str(),
        _ => return Err(CoreError::InvalidInput(format!("unknown page {page}"))),
    };
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| CoreError::Other(e.to_string()))
}

#[tauri::command]
pub async fn open_vm_folder(app: App<'_>) -> Result<()> {
    let folder = {
        let st = app.state.lock().await;
        st.vm
            .as_ref()
            .and_then(|v| {
                v.config_file
                    .as_ref()
                    .and_then(|c| c.parent().map(|p| p.to_path_buf()))
            })
            .or_else(|| st.choices.as_ref().map(|c| c.base_folder.clone()))
    }
    .ok_or_else(|| CoreError::InvalidInput("No VM folder yet.".into()))?;
    tauri_plugin_opener::open_path(folder, None::<&str>)
        .map_err(|e| CoreError::Other(e.to_string()))
}

#[tauri::command]
pub async fn delete_vm(app: App<'_>, confirm_name: String) -> Result<String> {
    setup::delete_vm(&app, &confirm_name).await
}

/// Forget setup progress without touching VirtualBox or any VM files.
#[tauri::command]
pub async fn forget_setup(app: App<'_>) -> Result<SetupState> {
    let mut st = app.state.lock().await;
    let fresh = SetupState::new(APP_VERSION);
    let old_vm = st.vm.clone();
    *st = fresh;
    if let Some(vm) = old_vm {
        st.note(format!(
            "previous setup forgotten; VM {} ({}) left untouched in VirtualBox",
            vm.name, vm.uuid
        ));
    }
    let s = st.clone();
    drop(st);
    app.save().await?;
    Ok(s)
}

#[tauri::command]
pub async fn list_cameras() -> Result<Vec<vmsa_core::media::Camera>> {
    let host = vmsa_core::host::inspect().await;
    let vb = vmsa_core::vbox::client::detect(host.os)
        .await?
        .ok_or(CoreError::VirtualBoxNotFound)?;
    let vbm = vmsa_core::vbox::client::VBoxManage::new(vb.vboxmanage);
    let out = vbm
        .run_raw(
            &["list".into(), "webcams".into()],
            std::time::Duration::from_secs(60),
            None,
        )
        .await?;
    Ok(vmsa_core::media::parse_cameras(&out.stdout))
}

#[tauri::command]
pub async fn media_action(app: App<'_>, action: vmsa_core::media::MediaAction) -> Result<String> {
    setup::media_action(&app, action).await
}
