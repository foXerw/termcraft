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

/// 解析模板字符串并校验版本，不写任何文件、不 apply。
/// 由前端拿到返回的 PresetTemplate 后弹冲突解决 Modal，再用
/// save_preset / save_preset_group 逐项应用。
pub fn parse_template(json: &str) -> Result<PresetTemplate, AppError> {
    let template: PresetTemplate = serde_json::from_str(json)
        .map_err(|e| AppError::Preset(format!("无法解析预设文件: {}", e)))?;
    if template.version != "1.0" {
        return Err(AppError::Preset(format!("不支持的预设文件版本: {}", template.version)));
    }
    Ok(template)
}