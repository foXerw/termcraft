use crate::errors::AppError;
use crate::preset::models::*;
use serde::{Deserialize, Serialize};

/// Template format for sharing presets. This is the on-disk file shape and is
/// used by `export_template` to serialize a clean `{ version, exported_at,
/// presets, groups }` document. It is NOT the parse return type — see
/// `ParsedTemplate` / `parse_template` for per-item tolerant parsing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetTemplate {
    pub version: String,
    pub exported_at: String,
    pub presets: Vec<Preset>,
    pub groups: Vec<PresetGroup>,
}

/// A preset that failed to deserialize — surfaced to the frontend so the
/// import dialog can show it greyed-out without aborting the whole import.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorruptedPreset {
    /// Position in the original presets array (for display ordering).
    pub index: usize,
    /// Best-effort name extracted from the raw value, if any (for the row label).
    pub name: Option<String>,
    /// Why it failed to parse.
    pub error: String,
}

/// Result of parsing an import file: valid presets + groups, plus any
/// presets that failed per-item deserialization (kept separate so the
/// frontend can grey them out instead of aborting the whole import).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedTemplate {
    pub version: String,
    pub exported_at: String,
    pub presets: Vec<Preset>,
    pub groups: Vec<PresetGroup>,
    pub corrupted: Vec<CorruptedPreset>,
}

/// Export presets to a JSON template string. Output format is the plain
/// `PresetTemplate` shape (`{ version, exported_at, presets, groups }`).
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

/// 解析模板字符串并校验版本，逐条解析预设（损坏的单独收集，不阻断整体）。
/// 由前端拿到返回的 ParsedTemplate 后弹冲突解决 Modal，再用
/// save_preset / save_preset_group 逐项应用。
pub fn parse_template(json: &str) -> Result<ParsedTemplate, AppError> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| AppError::Preset(format!("无法解析预设文件: {}", e)))?;

    let version = value.get("version").and_then(|v| v.as_str()).unwrap_or("");
    if version != "1.0" {
        return Err(AppError::Preset(format!("不支持的预设文件版本: {}", version)));
    }

    let exported_at = value
        .get("exported_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let groups: Vec<PresetGroup> = value
        .get("groups")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let raw_presets = value
        .get("presets")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut presets = Vec::new();
    let mut corrupted = Vec::new();
    for (i, raw) in raw_presets.iter().enumerate() {
        let name = raw.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
        match serde_json::from_value::<Preset>(raw.clone()) {
            Ok(p) => presets.push(p),
            Err(e) => corrupted.push(CorruptedPreset {
                index: i,
                name,
                error: e.to_string(),
            }),
        }
    }

    Ok(ParsedTemplate {
        version: version.to_string(),
        exported_at,
        presets,
        groups,
        corrupted,
    })
}
