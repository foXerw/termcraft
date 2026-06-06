use std::fs;
use std::path::PathBuf;

use crate::connection::ConnectionConfig;
use crate::errors::AppError;
use crate::preset::models::{Preset, PresetGroup};
use crate::preset::models::ScheduledTask;
use crate::config::models::AppSettings;

/// Get the data directory for TermCraft
fn data_dir() -> Result<PathBuf, AppError> {
    let base = dirs::data_dir()
        .ok_or(AppError::Config("Could not determine data directory".to_string()))?;
    let dir = base.join("termcraft");
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .map_err(|e| AppError::Config(format!("Failed to create data dir: {}", e)))?;
    }
    Ok(dir)
}

/// Atomic file write — write to temp file then rename
fn atomic_write(path: &PathBuf, content: &str) -> Result<(), AppError> {
    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, content)
        .map_err(|e| AppError::Config(format!("Failed to write temp file: {}", e)))?;
    fs::rename(&temp_path, path)
        .map_err(|e| AppError::Config(format!("Failed to rename temp file: {}", e)))?;
    Ok(())
}

/// Read and parse a JSON file
fn read_json<T: serde::de::DeserializeOwned>(path: &PathBuf) -> Result<Vec<T>, AppError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path)
        .map_err(|e| AppError::Config(format!("Failed to read file: {}", e)))?;
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&content)
        .map_err(|e| AppError::Config(format!("Failed to parse JSON: {}", e)))
}

/// Write a list to a JSON file
fn write_json<T: serde::Serialize>(path: &PathBuf, items: &[T]) -> Result<(), AppError> {
    let content = serde_json::to_string_pretty(items)
        .map_err(|e| AppError::Config(format!("Failed to serialize: {}", e)))?;
    atomic_write(path, &content)
}

// === Connection configs ===

pub fn load_connection_configs() -> Result<Vec<ConnectionConfig>, AppError> {
    let dir = data_dir()?;
    read_json::<ConnectionConfig>(&dir.join("connections.json"))
}

pub fn save_connection_configs(configs: &[ConnectionConfig]) -> Result<(), AppError> {
    let dir = data_dir()?;
    write_json(&dir.join("connections.json"), configs)
}

pub fn save_connection_config(config: &ConnectionConfig) -> Result<(), AppError> {
    let mut configs = load_connection_configs()?;
    // Update existing or add new
    if let Some(idx) = configs.iter().position(|c| c.id == config.id) {
        configs[idx] = config.clone();
    } else {
        configs.push(config.clone());
    }
    save_connection_configs(&configs)
}

pub fn delete_connection_config(id: &str) -> Result<(), AppError> {
    let mut configs = load_connection_configs()?;
    configs.retain(|c| c.id != id);
    save_connection_configs(&configs)
}

// === Presets ===

pub fn load_presets() -> Result<Vec<Preset>, AppError> {
    let dir = data_dir()?;
    read_json::<Preset>(&dir.join("presets.json"))
}

pub fn save_presets(presets: &[Preset]) -> Result<(), AppError> {
    let dir = data_dir()?;
    write_json(&dir.join("presets.json"), presets)
}

pub fn save_preset(preset: &Preset) -> Result<(), AppError> {
    let mut presets = load_presets()?;
    if let Some(idx) = presets.iter().position(|p| p.id == preset.id) {
        presets[idx] = preset.clone();
    } else {
        presets.push(preset.clone());
    }
    save_presets(&presets)
}

pub fn delete_preset(id: &str) -> Result<(), AppError> {
    let mut presets = load_presets()?;
    presets.retain(|p| p.id != id);
    save_presets(&presets)
}

// === Preset Groups ===

pub fn load_preset_groups() -> Result<Vec<PresetGroup>, AppError> {
    let dir = data_dir()?;
    read_json::<PresetGroup>(&dir.join("groups.json"))
}

pub fn save_preset_groups(groups: &[PresetGroup]) -> Result<(), AppError> {
    let dir = data_dir()?;
    write_json(&dir.join("groups.json"), groups)
}

pub fn save_preset_group(group: &PresetGroup) -> Result<(), AppError> {
    let mut groups = load_preset_groups()?;
    if let Some(idx) = groups.iter().position(|g| g.id == group.id) {
        groups[idx] = group.clone();
    } else {
        groups.push(group.clone());
    }
    save_preset_groups(&groups)
}

pub fn delete_preset_group(id: &str) -> Result<(), AppError> {
    let mut groups = load_preset_groups()?;
    groups.retain(|g| g.id != id);
    save_preset_groups(&groups)
}

// === Schedules ===

pub fn load_schedules() -> Result<Vec<ScheduledTask>, AppError> {
    let dir = data_dir()?;
    read_json::<ScheduledTask>(&dir.join("schedules.json"))
}

pub fn save_schedules(schedules: &[ScheduledTask]) -> Result<(), AppError> {
    let dir = data_dir()?;
    write_json(&dir.join("schedules.json"), schedules)
}

pub fn save_schedule(task: &ScheduledTask) -> Result<(), AppError> {
    let mut schedules = load_schedules()?;
    if let Some(idx) = schedules.iter().position(|s| s.id == task.id) {
        schedules[idx] = task.clone();
    } else {
        schedules.push(task.clone());
    }
    save_schedules(&schedules)
}

pub fn delete_schedule(id: &str) -> Result<(), AppError> {
    let mut schedules = load_schedules()?;
    schedules.retain(|s| s.id != id);
    save_schedules(&schedules)
}

// === Settings ===

pub fn load_settings() -> Result<AppSettings, AppError> {
    let dir = data_dir()?;
    let path = dir.join("settings.json");
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| AppError::Config(format!("Failed to read settings: {}", e)))?;
    if content.trim().is_empty() {
        return Ok(AppSettings::default());
    }
    serde_json::from_str(&content)
        .map_err(|e| AppError::Config(format!("Failed to parse settings: {}", e)))
}

pub fn save_settings(settings: &AppSettings) -> Result<(), AppError> {
    let dir = data_dir()?;
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| AppError::Config(format!("Failed to serialize settings: {}", e)))?;
    atomic_write(&dir.join("settings.json"), &content)
}