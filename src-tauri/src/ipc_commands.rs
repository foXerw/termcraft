use std::collections::HashMap;
use tokio::sync::Mutex;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;

use crate::connection::manager::ConnectionManager;
use crate::connection::ssh::SSHHandler;
use crate::connection::telnet::TelnetHandler;
use crate::connection::local::LocalShellHandler;
use crate::connection::{AuthConfig, ConnectionConfig, ConnectionInfo};
use crate::preset::engine::PresetEngine;
use crate::preset::models::*;
use crate::preset::template;
use crate::config::models::AppSettings;
use crate::config::store;

/// Global app state
pub struct AppState {
    pub connection_manager: ConnectionManager,
    pub preset_engine: Mutex<PresetEngine>,
}

// === Connection Commands ===

#[tauri::command]
pub async fn connect_ssh(
    id: String,
    host: String,
    port: u16,
    username: String,
    auth: AuthConfig,
    channel: Channel,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut handler = SSHHandler::new(id.clone(), host, port, username, auth);
    handler.connect()
        .await
        .map_err(|e| e.to_string())?;

    // Start forwarding data to frontend
    handler.start_forward_task(channel);

    state.connection_manager.register_ssh(id, handler).await;
    Ok(())
}

#[tauri::command]
pub async fn connect_telnet(
    id: String,
    host: String,
    port: u16,
    channel: Channel,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut handler = TelnetHandler::new(id.clone(), host, port);
    handler.connect(channel)
        .await
        .map_err(|e| e.to_string())?;

    state.connection_manager.register_telnet(id, handler).await;
    Ok(())
}

#[tauri::command]
pub async fn connect_local(
    id: String,
    shell: Option<String>,
    channel: Channel,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let shell_cmd = shell.unwrap_or_default();
    let mut handler = LocalShellHandler::new(id.clone(), shell_cmd);
    handler.connect(channel)
        .map_err(|e| e.to_string())?;

    state.connection_manager.register_local(id, handler).await;
    Ok(())
}

#[tauri::command]
pub async fn disconnect(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.connection_manager.remove(&id).await;
    Ok(())
}

#[tauri::command]
pub async fn write_to_connection(
    id: String,
    data: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.connection_manager.write_to(&id, &data)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn resize_connection(
    id: String,
    cols: u16,
    rows: u16,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.connection_manager.resize(&id, cols, rows)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_connections(
    state: State<'_, AppState>,
) -> Result<Vec<ConnectionInfo>, String> {
    Ok(state.connection_manager.list().await)
}

// === Connection Config Persistence ===

#[tauri::command]
pub async fn save_connection_config(config: ConnectionConfig) -> Result<(), String> {
    store::save_connection_config(&config).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_connection_config(id: String) -> Result<(), String> {
    store::delete_connection_config(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_connection_configs() -> Result<Vec<ConnectionConfig>, String> {
    store::load_connection_configs().map_err(|e| e.to_string())
}

// === Preset Commands ===

#[tauri::command]
pub async fn save_preset(preset: Preset) -> Result<(), String> {
    store::save_preset(&preset).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_preset(id: String) -> Result<(), String> {
    store::delete_preset(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_presets() -> Result<Vec<Preset>, String> {
    store::load_presets().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_preset_group(group: PresetGroup) -> Result<(), String> {
    store::save_preset_group(&group).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_preset_group(id: String) -> Result<(), String> {
    store::delete_preset_group(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_preset_groups() -> Result<Vec<PresetGroup>, String> {
    store::load_preset_groups().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn execute_preset(
    exec_id: String,
    preset_id: String,
    connection_id: String,
    variables: HashMap<String, String>,
    status_channel: Channel,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let presets = store::load_presets().map_err(|e| e.to_string())?;
    let preset = presets.iter()
        .find(|p| p.id == preset_id)
        .cloned()
        .ok_or(format!("Preset {} not found", preset_id))?;

    state.preset_engine.lock().await
        .execute(exec_id, preset, connection_id, variables, &state.connection_manager, &status_channel)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_preset(
    exec_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.preset_engine.lock().await
        .stop(&exec_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pause_preset(
    exec_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.preset_engine.lock().await
        .pause(&exec_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn resume_preset(
    exec_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.preset_engine.lock().await
        .resume(&exec_id)
        .await
        .map_err(|e| e.to_string())
}

// === Schedule Commands ===

#[tauri::command]
pub async fn create_schedule(task: ScheduledTask) -> Result<(), String> {
    store::save_schedule(&task).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_schedule(id: String) -> Result<(), String> {
    store::delete_schedule(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_schedules() -> Result<Vec<ScheduledTask>, String> {
    store::load_schedules().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_schedule(id: String, enabled: bool) -> Result<(), String> {
    let mut schedules = store::load_schedules().map_err(|e| e.to_string())?;
    if let Some(schedule) = schedules.iter_mut().find(|s| s.id == id) {
        schedule.enabled = enabled;
        store::save_schedules(&schedules).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// === Template Commands ===

#[tauri::command]
pub async fn export_template(preset_ids: Vec<String>) -> Result<String, String> {
    let presets = store::load_presets().map_err(|e| e.to_string())?;
    let selected: Vec<Preset> = presets.iter()
        .filter(|p| preset_ids.contains(&p.id))
        .cloned()
        .collect();
    let groups = store::load_preset_groups().map_err(|e| e.to_string())?;
    let group_ids: Vec<String> = selected.iter()
        .filter_map(|p| p.group_id.clone())
        .collect();
    let selected_groups: Vec<PresetGroup> = groups.iter()
        .filter(|g| group_ids.contains(&g.id))
        .cloned()
        .collect();
    template::export_template(selected, selected_groups).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_template(json: String, overwrite: bool) -> Result<Vec<String>, String> {
    let existing_presets = store::load_presets().map_err(|e| e.to_string())?;
    let existing_groups = store::load_preset_groups().map_err(|e| e.to_string())?;
    let (new_presets, new_groups) = template::import_template(&json, &existing_presets, &existing_groups, overwrite)
        .map_err(|e| e.to_string())?;

    for p in &new_presets {
        store::save_preset(p).map_err(|e| e.to_string())?;
    }
    for g in &new_groups {
        store::save_preset_group(g).map_err(|e| e.to_string())?;
    }

    Ok(new_presets.iter().map(|p| p.id.clone()).collect())
}

// === Settings Commands ===

#[tauri::command]
pub async fn load_settings() -> Result<AppSettings, String> {
    store::load_settings().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_settings(settings: AppSettings) -> Result<(), String> {
    store::save_settings(&settings).map_err(|e| e.to_string())
}