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
use crate::reachability::ReachabilityService;

/// Global app state
pub struct AppState {
    pub connection_manager: ConnectionManager,
    pub preset_engine: Mutex<PresetEngine>,
    pub reachability: std::sync::Arc<ReachabilityService>,
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
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut handler = SSHHandler::new(id.clone(), host, port, username, auth, app);
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
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut handler = TelnetHandler::new(id.clone(), host, port, app);
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
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let shell_cmd = shell.unwrap_or_default();
    let mut handler = LocalShellHandler::new(id.clone(), shell_cmd, app);
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

/// Replace the set of connections the reachability scheduler probes.
/// Each entry: (id, host, port). Frontend derives these from its config list
/// (only connections that have a host, i.e. SSH/Telnet).
#[tauri::command]
pub async fn set_reachability_targets(
    targets: Vec<(String, String, u16)>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.reachability.set_targets(targets).await;
    Ok(())
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

// === Preset Import/Export ===

#[tauri::command]
pub async fn export_presets_to_file(path: String, preset_ids: Vec<String>) -> Result<(), String> {
    let presets = store::load_presets().map_err(|e| e.to_string())?;
    let groups = store::load_preset_groups().map_err(|e| e.to_string())?;

    // preset_ids 为空 => 导出全部
    let selected_presets: Vec<Preset> = if preset_ids.is_empty() {
        presets
    } else {
        presets.iter().filter(|p| preset_ids.contains(&p.id)).cloned().collect()
    };
    if !preset_ids.is_empty() && selected_presets.len() != preset_ids.len() {
        return Err("部分预设未找到".to_string());
    }

    // 带上所选预设各自所属的分组
    let group_ids: Vec<String> = selected_presets
        .iter()
        .filter_map(|p| p.group_id.clone())
        .collect();
    let selected_groups: Vec<PresetGroup> = groups
        .iter()
        .filter(|g| group_ids.contains(&g.id))
        .cloned()
        .collect();

    let json = template::export_template(selected_presets, selected_groups)
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("写入文件失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn parse_template_file(path: String) -> Result<template::PresetTemplate, String> {
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("读取文件失败: {}", e))?;
    template::parse_template(&content).map_err(|e| e.to_string())
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