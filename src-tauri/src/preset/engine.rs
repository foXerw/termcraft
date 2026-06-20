use std::collections::HashMap;
use std::time::{Duration, Instant};

use regex::Regex;
use tauri::ipc::Channel;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::connection::manager::ConnectionManager;
use crate::errors::AppError;
use crate::preset::models::*;

/// Preset execution engine.
///
/// The engine writes commands to a connection via `ConnectionManager::write_to`
/// and, when a `WaitCondition` is set, reads the command's echoed output back
/// through the connection's output tap to decide success/failure, branching per
/// `on_fail`.
pub struct PresetEngine {
    cancelled: tokio::sync::Mutex<HashMap<String, bool>>,
}

/// Outcome of evaluating one command.
enum CommandOutcome {
    /// Expectation met (or no wait configured): proceed.
    Ok,
    /// Failed: write error, expectation unmet, regex error, etc.
    Failed(String),
}

impl PresetEngine {
    pub fn new() -> Self {
        Self {
            cancelled: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Check if execution has been cancelled
    async fn check_cancelled(&self, exec_id: &str) -> bool {
        *self.cancelled.lock().await.get(exec_id).unwrap_or(&false)
    }

    /// Execute a preset on a connection.
    pub async fn execute(
        &self,
        exec_id: String,
        preset: Preset,
        connection_id: String,
        variables: HashMap<String, String>,
        manager: &ConnectionManager,
        status_channel: &Channel,
    ) -> Result<(), AppError> {
        self.cancelled.lock().await.insert(exec_id.clone(), false);
        let preset_id = preset.id.clone();

        // Filter enabled commands + substitute variables.
        let commands: Vec<CommandItem> = preset
            .commands
            .iter()
            .filter(|c| c.enabled)
            .cloned()
            .map(|c| self.substitute_variables(&c, &variables))
            .collect();

        let total = commands.len();
        if total == 0 {
            self.send_status(
                status_channel, &exec_id, &preset_id, ExecutionState::Completed, 0, total, 0,
                Some("No commands to execute".to_string()), None, None,
            );
            self.cancelled.lock().await.remove(&exec_id);
            return Ok(());
        }

        if self.check_cancelled(&exec_id).await {
            self.send_status(
                status_channel, &exec_id, &preset_id, ExecutionState::Cancelled, 0, total, 0,
                Some("Cancelled before start".to_string()), None, None,
            );
            self.cancelled.lock().await.remove(&exec_id);
            return Ok(());
        }

        // Subscribe to the connection's output so we can capture per-command
        // output for matching. Unsubscribed in all exit paths.
        let (sub_id, rx) = match manager.subscribe_output(&connection_id).await {
            Some(pair) => pair,
            None => {
                self.send_status(
                    status_channel, &exec_id, &preset_id, ExecutionState::Failed, 0, total, 0,
                    Some("Connection output stream unavailable".to_string()), None, None,
                );
                self.cancelled.lock().await.remove(&exec_id);
                return Ok(());
            }
        };

        let result = self
            .run_dispatch(&exec_id, &preset_id, &commands, &preset.execution_mode,
                           &connection_id, manager, rx, status_channel)
            .await;

        // Always release the output subscriber.
        manager.unsubscribe_output(&connection_id, sub_id).await;
        self.cancelled.lock().await.remove(&exec_id);
        result
    }

    /// Dispatch to the configured execution mode. Returns Ok(()) always; terminal
    /// status (Completed/Failed/Cancelled) is emitted from here.
    async fn run_dispatch(
        &self,
        exec_id: &str,
        preset_id: &str,
        commands: &[CommandItem],
        mode: &ExecutionMode,
        connection_id: &str,
        manager: &ConnectionManager,
        mut rx: UnboundedReceiver<Vec<u8>>,
        status_channel: &Channel,
    ) -> Result<(), AppError> {
        match mode {
            ExecutionMode::Single => {
                let aborted = self
                    .run_commands(exec_id, preset_id, &commands[..1], connection_id, 0,
                                  manager, &mut rx, status_channel).await?;
                if !aborted {
                    self.send_status(status_channel, exec_id, preset_id,
                        ExecutionState::Completed, commands.len(), commands.len(), 0,
                        Some("Completed".to_string()), None, None);
                }
            }
            ExecutionMode::Batch { .. } => {
                let aborted = self
                    .run_commands(exec_id, preset_id, commands, connection_id, 0,
                                  manager, &mut rx, status_channel).await?;
                if !aborted {
                    self.send_status(status_channel, exec_id, preset_id,
                        ExecutionState::Completed, commands.len(), commands.len(), 0,
                        Some("Batch completed".to_string()), None, None);
                }
            }
            ExecutionMode::Loop { count, interval_ms, .. } => {
                let max = count.unwrap_or(u32::MAX);
                let mut loop_count: u32 = 0;
                while loop_count < max {
                    if self.check_cancelled(exec_id).await {
                        self.send_status(status_channel, exec_id, preset_id,
                            ExecutionState::Cancelled, 0, commands.len(), loop_count,
                            Some("Cancelled".to_string()), None, None);
                        return Ok(());
                    }
                    self.send_status(status_channel, exec_id, preset_id,
                        ExecutionState::Running, 0, commands.len(), loop_count,
                        Some(format!("Loop iteration {}", loop_count + 1)), None, None);

                    let aborted = self
                        .run_commands(exec_id, preset_id, commands, connection_id, loop_count,
                                      manager, &mut rx, status_channel).await?;
                    if aborted {
                        return Ok(());
                    }

                    loop_count += 1;
                    if *interval_ms > 0 && loop_count < max {
                        tokio::time::sleep(Duration::from_millis(*interval_ms)).await;
                    }
                }
                self.send_status(status_channel, exec_id, preset_id,
                    ExecutionState::Completed, commands.len(), commands.len(), loop_count,
                    Some("Loop completed".to_string()), None, None);
            }
        }
        Ok(())
    }

    /// Run a sequence of commands, evaluating wait conditions and branching on
    /// failure. Returns `Ok(true)` if the run was aborted/cancelled (terminal
    /// status already emitted), `Ok(false)` if it completed normally.
    async fn run_commands(
        &self,
        exec_id: &str,
        preset_id: &str,
        commands: &[CommandItem],
        connection_id: &str,
        loop_count: u32,
        manager: &ConnectionManager,
        rx: &mut UnboundedReceiver<Vec<u8>>,
        status_channel: &Channel,
    ) -> Result<bool, AppError> {
        let mut buffer = String::new();
        let total = commands.len();

        for (i, cmd) in commands.iter().enumerate() {
            if self.check_cancelled(exec_id).await {
                self.send_status(status_channel, exec_id, preset_id,
                    ExecutionState::Cancelled, i, total, loop_count,
                    Some("Cancelled".to_string()), None, None);
                return Ok(true);
            }

            self.send_status(status_channel, exec_id, preset_id,
                ExecutionState::Running, i, total, loop_count,
                Some(format!("Executing [{}/{}]: {}", i + 1, total, cmd.command)),
                None, None);

            // Water level: only match output produced AFTER we send this command,
            // so a lingering echo from a previous command can't satisfy the match.
            let start = buffer.len();

            let write_res = manager
                .write_to(connection_id, &(cmd.command.clone() + "\n"))
                .await;
            let mut outcome = match write_res {
                Ok(()) => CommandOutcome::Ok,
                Err(e) => CommandOutcome::Failed(format!("write error: {}", e)),
            };

            if matches!(outcome, CommandOutcome::Ok) {
                if let Some(wait) = cmd.wait_for.as_ref() {
                    outcome = self.wait_for_condition(exec_id, rx, &mut buffer, start, wait).await;
                }
                if cmd.delay_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(cmd.delay_ms)).await;
                }
            }

            match outcome {
                CommandOutcome::Ok => {
                    self.send_status(status_channel, exec_id, preset_id,
                        ExecutionState::Running, i, total, loop_count,
                        Some(format!("[{}/{}] ok", i + 1, total)),
                        Some(true), Some(snippet(&buffer, start)));
                }
                CommandOutcome::Failed(reason) => {
                    let snippet = snippet(&buffer, start);
                    match cmd.on_fail {
                        OnFail::Abort => {
                            self.send_status(status_channel, exec_id, preset_id,
                                ExecutionState::Failed, i, total, loop_count,
                                Some(format!("第 {} 条失败，已中止: {}", i + 1, reason)),
                                Some(false), Some(snippet));
                            return Ok(true);
                        }
                        OnFail::Continue => {
                            self.send_status(status_channel, exec_id, preset_id,
                                ExecutionState::Running, i, total, loop_count,
                                Some(format!("[{}/{}] 失败但继续: {}", i + 1, total, reason)),
                                Some(false), Some(snippet));
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    /// Wait for a condition on the output stream. Drains `buffer` with received
    /// bytes, matches the segment after `start`, and resolves the command
    /// outcome once matched or once the timeout window elapses.
    async fn wait_for_condition(
        &self,
        exec_id: &str,
        rx: &mut UnboundedReceiver<Vec<u8>>,
        buffer: &mut String,
        start: usize,
        wait: &WaitCondition,
    ) -> CommandOutcome {
        let re = match wait.match_type {
            MatchType::Regex => match Regex::new(&wait.pattern) {
                Ok(r) => Some(r),
                Err(e) => return CommandOutcome::Failed(format!("invalid regex '{}': {}", wait.pattern, e)),
            },
            _ => None,
        };

        let timeout = wait.timeout_ms.max(1);
        let deadline = Instant::now() + Duration::from_millis(timeout);

        loop {
            // Drain everything currently buffered.
            while let Ok(bytes) = rx.try_recv() {
                buffer.push_str(&String::from_utf8_lossy(&bytes));
            }

            let segment = buffer.get(start..).unwrap_or("");
            let found = match wait.match_type {
                MatchType::Contains => segment.contains(wait.pattern.as_str()),
                MatchType::Exact => segment.lines().any(|l| l.trim() == wait.pattern),
                MatchType::Regex => re.as_ref().map_or(false, |r| r.is_match(segment)),
            };

            if found {
                // Pattern appeared — resolve by expectation.
                return match wait.expect {
                    WaitExpect::Found => CommandOutcome::Ok,
                    WaitExpect::NotFound => CommandOutcome::Failed(format!("'{}' appeared (expected NotFound)", wait.pattern)),
                };
            }

            if self.check_cancelled(exec_id).await {
                return CommandOutcome::Failed("cancelled".to_string());
            }
            if Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        // Timeout: pattern never appeared during the window.
        match wait.expect {
            WaitExpect::Found => CommandOutcome::Failed(format!("timeout: '{}' not found within {}ms", wait.pattern, timeout)),
            WaitExpect::NotFound => CommandOutcome::Ok,
        }
    }

    /// Substitute {{variable}} patterns in a command.
    fn substitute_variables(&self, item: &CommandItem, variables: &HashMap<String, String>) -> CommandItem {
        let mut command = item.command.clone();
        for (name, value) in variables {
            command = command.replace(&format!("{{{{{}}}}}", name), value);
        }
        CommandItem {
            command,
            delay_ms: item.delay_ms,
            wait_for: item.wait_for.clone(),
            on_fail: item.on_fail,
            enabled: item.enabled,
            // id intentionally dropped — not needed at runtime.
            id: item.id.clone(),
        }
    }

    /// Send execution status to frontend.
    #[allow(clippy::too_many_arguments)]
    fn send_status(
        &self,
        channel: &Channel,
        exec_id: &str,
        preset_id: &str,
        state: ExecutionState,
        current_command_index: usize,
        total_commands: usize,
        current_loop: u32,
        message: Option<String>,
        command_succeeded: Option<bool>,
        captured_snippet: Option<String>,
    ) {
        let status = PresetExecutionStatus {
            exec_id: exec_id.to_string(),
            preset_id: preset_id.to_string(),
            state,
            current_command_index,
            total_commands,
            current_loop,
            message,
            command_succeeded,
            captured_snippet,
        };
        if let Ok(json) = serde_json::to_string(&status) {
            use tauri::ipc::InvokeResponseBody;
            let _ = channel.send(InvokeResponseBody::Json(json));
        }
    }

    /// Stop a running preset execution.
    pub async fn stop(&self, exec_id: &str) -> Result<(), AppError> {
        self.cancelled.lock().await.insert(exec_id.to_string(), true);
        Ok(())
    }

    /// Pause a running execution (TODO)
    pub async fn pause(&self, _exec_id: &str) -> Result<(), AppError> {
        Err(AppError::Preset("Pause/resume not yet fully implemented".to_string()))
    }

    /// Resume a paused execution
    pub async fn resume(&self, _exec_id: &str) -> Result<(), AppError> {
        Err(AppError::Preset("Pause/resume not yet fully implemented".to_string()))
    }
}

/// Tail of the captured output segment (last 512 chars) for quick debugging.
fn snippet(buffer: &str, start: usize) -> String {
    let seg = buffer.get(start..).unwrap_or("");
    let chars: Vec<char> = seg.chars().collect();
    if chars.len() > 512 {
        chars[chars.len() - 512..].iter().collect()
    } else {
        seg.to_string()
    }
}
