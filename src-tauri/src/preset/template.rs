use crate::errors::AppError;
use crate::preset::models::*;
use serde::{Deserialize, Serialize};

/// Template format for sharing presets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetTemplate {
    pub version: String,
    pub exported_at: String,
    pub presets: Vec<Preset>,
    pub groups: Vec<PresetGroup>,
}

/// Export presets to a JSON template string
pub fn export_template(presets: Vec<Preset>, groups: Vec<PresetGroup>) -> Result<String, AppError> {
    let template = PresetTemplate {
        version: "1.0".to_string(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        presets,
        groups,
    };
    serde_json::to_string_pretty(&template)
        .map_err(|e| AppError::Preset(format!("Failed to export template: {}", e)))
}

/// Import presets from a JSON template string
pub fn import_template(json: &str, existing_presets: &[Preset], existing_groups: &[PresetGroup], overwrite: bool) -> Result<(Vec<Preset>, Vec<PresetGroup>), AppError> {
    let template: PresetTemplate = serde_json::from_str(json)
        .map_err(|e| AppError::Preset(format!("Failed to parse template: {}", e)))?;

    // Version check
    if template.version != "1.0" {
        return Err(AppError::Preset(format!("Unsupported template version: {}", template.version)));
    }

    let mut new_presets = template.presets;
    let mut new_groups = template.groups;

    if !overwrite {
        // Filter out presets/groups that already exist by ID
        let existing_ids: Vec<String> = existing_presets.iter().map(|p| p.id.clone()).collect();
        let existing_group_ids: Vec<String> = existing_groups.iter().map(|g| g.id.clone()).collect();

        new_presets.retain(|p| !existing_ids.contains(&p.id));
        new_groups.retain(|g| !existing_group_ids.contains(&g.id));
    }

    Ok((new_presets, new_groups))
}