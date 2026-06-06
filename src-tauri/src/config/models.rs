use serde::{Deserialize, Serialize};

use crate::connection::{AuthConfig, ConnType};

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme: String,                // "dark" or "light"
    pub font_size: u16,               // Terminal font size
    pub font_family: String,          // Terminal font family
    pub default_cols: u16,            // Default terminal columns
    pub default_rows: u16,            // Default terminal rows
    pub scrollback: u32,              // Scrollback buffer size
    pub cursor_style: String,         // "block" / "underline" / "bar"
    pub locale: String,               // UI language
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            font_size: 14,
            font_family: "Consolas, 'Courier New', monospace".to_string(),
            default_cols: 80,
            default_rows: 24,
            scrollback: 5000,
            cursor_style: "block".to_string(),
            locale: "zh-CN".to_string(),
        }
    }
}