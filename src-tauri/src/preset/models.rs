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
    /// What to do when this command "fails" (write error, or wait_for
    /// expectation unmet within timeout). Default: Abort when a wait_for is
    /// set, Continue otherwise (applied at load/exec time).
    #[serde(default)]
    pub on_fail: OnFail,
    pub enabled: bool,
}

/// What to do when a command fails during a preset run.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum OnFail {
    /// Stop the whole preset (report Failed).
    Abort,
    /// Log the failure and continue to the next command.
    Continue,
}

impl Default for OnFail {
    fn default() -> Self {
        OnFail::Continue
    }
}

/// Wait condition — match output before proceeding to next command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitCondition {
    pub pattern: String,
    pub timeout_ms: u64,
    pub match_type: MatchType,
    /// Whether finding the pattern means success (Found) or failure (NotFound).
    /// E.g. `ps` must show a process → Found; an error string must NOT appear → NotFound.
    #[serde(default)]
    pub expect: WaitExpect,
}

/// Whether a match denotes success or failure.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum WaitExpect {
    Found,
    NotFound,
}

impl Default for WaitExpect {
    fn default() -> Self {
        WaitExpect::Found
    }
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
    /// Outcome of the command that just ran (when known). Lets the UI show
    /// "matched / not matched" per step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_succeeded: Option<bool>,
    /// Tail of the captured output for the command, for quick debugging.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_snippet: Option<String>,
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