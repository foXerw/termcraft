mod connection;
mod preset;
mod config;
mod errors;
mod ipc_commands;
mod security;
mod reachability;

use connection::manager::ConnectionManager;
use ipc_commands::AppState;
use preset::engine::PresetEngine;
use reachability::init as init_reachability;
use tauri::Manager;
use tokio::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let reachability = init_reachability();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            connection_manager: ConnectionManager::new(),
            preset_engine: Mutex::new(PresetEngine::new()),
            reachability: reachability.clone(),
        })
        .setup(move |app| {
            // Spawn the reachability scheduler; it owns an Arc to the service
            // shared with AppState and uses the app handle to emit events.
            reachability.clone().spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Connection commands
            ipc_commands::connect_ssh,
            ipc_commands::connect_telnet,
            ipc_commands::connect_local,
            ipc_commands::disconnect,
            ipc_commands::write_to_connection,
            ipc_commands::resize_connection,
            ipc_commands::list_connections,
            // Connection config persistence
            ipc_commands::save_connection_config,
            ipc_commands::delete_connection_config,
            ipc_commands::load_connection_configs,
            // Reachability
            ipc_commands::set_reachability_targets,
            // Preset commands
            ipc_commands::save_preset,
            ipc_commands::delete_preset,
            ipc_commands::load_presets,
            ipc_commands::save_preset_group,
            ipc_commands::delete_preset_group,
            ipc_commands::load_preset_groups,
            ipc_commands::execute_preset,
            ipc_commands::stop_preset,
            ipc_commands::pause_preset,
            ipc_commands::resume_preset,
            // Schedule commands
            ipc_commands::create_schedule,
            ipc_commands::delete_schedule,
            ipc_commands::load_schedules,
            ipc_commands::toggle_schedule,
            // Template commands
            ipc_commands::export_template,
            ipc_commands::import_template,
            // Settings commands
            ipc_commands::load_settings,
            ipc_commands::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}