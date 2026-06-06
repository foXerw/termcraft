use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A preset command collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub group_id: Option<String>,
    pub description: Option<String>,
    pub commands: Vec<CommandItem>,
    pub variables: Vec<Variable>,
    pub execution_mode: ExecutionMode,
    pub created_at: String,
    pub updated_at: String,
}

/// A single command item within a preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandItem {
    pub id: String,
    pub command: String,
    pub delay_ms: u64,
    pub wait_for: Option<WaitCondition>,
    pub enabled: bool,
}

/// Wait condition — match output before proceeding to next command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitCondition {
    pub pattern: String,
    pub timeout_ms: u64,
    pub match_type: MatchType,
}

/// How to match the wait condition pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchType {
    Exact,
    Contains,
    Regex,
}

/// Execution mode for preset — uses internally tagged format for clean JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum ExecutionMode {
    Single,
    Batch { stop_on_error: bool },
    Loop {
        count: Option<u32>,
        interval_ms: u64,
        stop_on_error: bool,
    },
}

/// A variable that can be substituted in commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    pub default_value: Option<String>,
    pub description: Option<String>,
}

/// A group/category for presets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetGroup {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<String>,
}

/// Preset execution status — sent to frontend via Channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetExecutionStatus {
    pub exec_id: String,
    pub preset_id: String,
    pub state: ExecutionState,
    pub current_command_index: usize,
    pub total_commands: usize,
    pub current_loop: u32,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionState {
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

/// A scheduled task that runs a preset at specified times
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: String,
    pub preset_id: String,
    pub connection_id: String,
    pub variables: HashMap<String, String>,
    pub schedule: Schedule,
    pub enabled: bool,
}

/// Schedule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum Schedule {
    Cron { expression: String },
    Interval { seconds: u64 },
    Once { at: String },
}