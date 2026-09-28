//! Tauri application layer: a narrow set of commands that the UI may call. All logic lives in
//! `vmsa-core`; this crate only wires it to the window, persists state and emits progress events.

mod commands;
mod setup;

use std::sync::Arc;

use tauri::Manager;
use tokio::sync::Mutex;

use vmsa_core::cmd::tokio_util_lite::CancellationToken;
use vmsa_core::paths::AppPaths;
use vmsa_core::state::{InstanceLock, SetupState};

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct AppState {
    pub paths: AppPaths,
    pub state: Mutex<SetupState>,
    /// Set while a long operation runs; a second operation is refused (`CoreError::Busy`).
    pub operation: Mutex<Option<RunningOperation>>,
    pub _lock: Mutex<Option<InstanceLock>>,
    pub startup_warning: Mutex<Option<String>>,
}

pub struct RunningOperation {
    pub name: String,
    pub cancel: CancellationToken,
}

impl AppState {
    pub async fn save(&self) -> Result<(), vmsa_core::CoreError> {
        let st = self.state.lock().await;
        st.save(&self.paths.state_file())
    }

    /// Begin an exclusive operation. Returns its cancellation token.
    pub async fn begin(&self, name: &str) -> Result<CancellationToken, vmsa_core::CoreError> {
        let mut op = self.operation.lock().await;
        if op.is_some() {
            return Err(vmsa_core::CoreError::Busy);
        }
        let token = CancellationToken::new();
        *op = Some(RunningOperation {
            name: name.to_string(),
            cancel: token.clone(),
        });
        Ok(token)
    }

    pub async fn end(&self) {
        *self.operation.lock().await = None;
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let paths = AppPaths::discover().expect("could not determine application directories");
    paths
        .ensure()
        .expect("could not create application directories");

    let (state, warning) = SetupState::load_or_recover(&paths.state_file(), APP_VERSION);
    let lock = InstanceLock::acquire(&paths.lock_file()).ok();
    let startup_warning = if lock.is_none() {
        Some("Another copy of VM Setup Assistant seems to be running. Setup actions are disabled here until it closes.".to_string())
    } else {
        warning
    };

    let app_state = Arc::new(AppState {
        paths: paths.clone(),
        state: Mutex::new(state),
        operation: Mutex::new(None),
        _lock: Mutex::new(lock),
        startup_warning: Mutex::new(startup_warning),
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // A second launch just focuses the existing window.
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Folder {
                        path: paths.logs_dir.clone(),
                        file_name: Some("vm-setup-assistant".into()),
                    },
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .level(log::LevelFilter::Info)
                .max_file_size(2_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(3))
                .build(),
        )
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::get_state,
            commands::inspect_host,
            commands::inspect_iso,
            commands::set_choices,
            commands::resolve_virtualbox_release,
            commands::download_virtualbox,
            commands::use_existing_virtualbox_installer,
            commands::run_virtualbox_installer,
            commands::redetect_virtualbox,
            commands::cancel_operation,
            commands::create_vm,
            commands::vm_status,
            commands::start_vm,
            commands::shutdown_vm,
            commands::save_state_vm,
            commands::power_off_vm,
            commands::confirm_guest_item,
            commands::eject_install_iso,
            commands::network_diagnose,
            commands::network_apply_repair,
            commands::build_support_report,
            commands::save_support_report,
            commands::open_official_page,
            commands::open_vm_folder,
            commands::set_stage,
            commands::delete_vm,
            commands::forget_setup,
            commands::acknowledge_startup_warning,
        ])
        .run(tauri::generate_context!())
        .expect("error while running VM Setup Assistant");
}
