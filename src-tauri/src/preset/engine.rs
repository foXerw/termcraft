use std::collections::HashMap;
use tokio::sync::Mutex;
use tauri::ipc::{Channel, InvokeResponseBody};

use crate::connection::manager::ConnectionManager;
use crate::errors::AppError;
use crate::preset::models::*;

/// Preset execution engine
pub struct PresetEngine {
    cancelled: Mutex<HashMap<String, bool>>,
}

impl PresetEngine {
    pub fn new() -> Self {
        Self {
            cancelled: Mutex::new(HashMap::new()),
        }
    }

    fn is_cancelled(&self, exec_id: &str) -> bool {
        // Quick check without blocking — may be slightly stale but acceptable
        false
    }

    /// Execute a preset on a connection
    pub async fn execute(
        &self,
        exec_id: String,
        preset: Preset,
        connection_id: String,
        variables: HashMap<String, String>,
        manager: &ConnectionManager,
        status_channel: &Channel,
    ) -> Result<(), AppError> {
        // Mark as not cancelled
        self.cancelled.lock().await.insert(exec_id.clone(), false);

        // Extract preset ID for later use
        let preset_id = preset.id.clone();

        // Filter enabled commands and apply variable substitution
        let commands: Vec<CommandItem> = preset.commands
            .iter()
            .filter(|c| c.enabled)
            .cloned()
            .map(|c| self.substitute_variables(&c, &variables))
            .collect();

        let total = commands.len();
        if total == 0 {
            self.send_status(status_channel, &exec_id, preset_id.as_str(),
                ExecutionState::Completed, 0, total, 0, Some("No commands to execute".to_string()));
            self.cancelled.lock().await.remove(&exec_id);
            return Ok(());
        }

        let cancelled_map = self.cancelled.lock().await;
        let is_cancelled = *cancelled_map.get(&exec_id).unwrap_or(&false);
        drop(cancelled_map);

        if is_cancelled {
            self.send_status(status_channel, &exec_id, preset_id.as_str(),
                ExecutionState::Cancelled, 0, total, 0, Some("Cancelled before start".to_string()));
            self.cancelled.lock().await.remove(&exec_id);
            return Ok(());
        }

        match &preset.execution_mode {
            ExecutionMode::Single => {
                self.execute_single(&exec_id, preset_id.as_str(), &commands, connection_id.as_str(), manager, status_channel).await?;
            }
            ExecutionMode::Batch { stop_on_error } => {
                self.execute_batch(&exec_id, preset_id.as_str(), &commands, connection_id.as_str(), manager, status_channel, *stop_on_error).await?;
            }
            ExecutionMode::Loop { count, interval_ms, stop_on_error } => {
                self.execute_loop(&exec_id, preset_id.as_str(), &commands, connection_id.as_str(), manager, status_channel, *count, *interval_ms, *stop_on_error).await?;
            }
        }

        self.cancelled.lock().await.remove(&exec_id);
        Ok(())
    }

    /// Substitute {{variable}} patterns in a command
    fn substitute_variables(&self, item: &CommandItem, variables: &HashMap<String, String>) -> CommandItem {
        let mut command = item.command.clone();
        for (name, value) in variables {
            command = command.replace(&format!("{{{{{}}}}}", name), value);
        }
        CommandItem {
            id: item.id.clone(),
            command,
            delay_ms: item.delay_ms,
            wait_for: item.wait_for.clone(),
            enabled: item.enabled,
        }
    }

    /// Check if execution has been cancelled
    async fn check_cancelled(&self, exec_id: &str) -> bool {
        *self.cancelled.lock().await.get(exec_id).unwrap_or(&false)
    }

    /// Execute single command
    async fn execute_single(
        &self,
        exec_id: &str,
        preset_id: &str,
        commands: &[CommandItem],
        connection_id: &str,
        manager: &ConnectionManager,
        status_channel: &Channel,
    ) -> Result<(), AppError> {
        if self.check_cancelled(exec_id).await {
            self.send_status(status_channel, exec_id, preset_id,
                ExecutionState::Cancelled, 0, commands.len(), 0, Some("Cancelled".to_string()));
            return Ok(());
        }

        let cmd = &commands[0];
        self.send_status(status_channel, exec_id, preset_id,
            ExecutionState::Running, 0, commands.len(), 0,
            Some(format!("Executing: {}", cmd.command)));

        manager.write_to(connection_id, &(cmd.command.clone() + "\n")).await?;

        // Wait for delay
        if cmd.delay_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(cmd.delay_ms)).await;
        }

        self.send_status(status_channel, exec_id, preset_id,
            ExecutionState::Completed, 1, commands.len(), 0, Some("Completed".to_string()));
        Ok(())
    }

    /// Execute batch of commands sequentially
    async fn execute_batch(
        &self,
        exec_id: &str,
        preset_id: &str,
        commands: &[CommandItem],
        connection_id: &str,
        manager: &ConnectionManager,
        status_channel: &Channel,
        stop_on_error: bool,
    ) -> Result<(), AppError> {
        for (i, cmd) in commands.iter().enumerate() {
            if self.check_cancelled(exec_id).await {
                self.send_status(status_channel, exec_id, preset_id,
                    ExecutionState::Cancelled, i, commands.len(), 0, Some("Cancelled".to_string()));
                return Ok(());
            }

            self.send_status(status_channel, exec_id, preset_id,
                ExecutionState::Running, i, commands.len(), 0,
                Some(format!("Executing [{}/{}]: {}", i + 1, commands.len(), cmd.command)));

            let write_result = manager.write_to(connection_id, &(cmd.command.clone() + "\n")).await;
            if write_result.is_err() && stop_on_error {
                self.send_status(status_channel, exec_id, preset_id,
                    ExecutionState::Failed, i, commands.len(), 0,
                    Some(format!("Failed at command {}: {}", i + 1, write_result.unwrap_err())));
                return Ok(());
            }

            // Delay between commands
            if cmd.delay_ms > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(cmd.delay_ms)).await;
            }
        }

        self.send_status(status_channel, exec_id, preset_id,
            ExecutionState::Completed, commands.len(), commands.len(), 0, Some("Batch completed".to_string()));
        Ok(())
    }

    /// Execute commands in a loop
    async fn execute_loop(
        &self,
        exec_id: &str,
        preset_id: &str,
        commands: &[CommandItem],
        connection_id: &str,
        manager: &ConnectionManager,
        status_channel: &Channel,
        count: Option<u32>,
        interval_ms: u64,
        stop_on_error: bool,
    ) -> Result<(), AppError> {
        let max_loops = count.unwrap_or(u32::MAX);
        let mut loop_count: u32 = 0;

        while loop_count < max_loops {
            if self.check_cancelled(exec_id).await {
                self.send_status(status_channel, exec_id, preset_id,
                    ExecutionState::Cancelled, 0, commands.len(), loop_count, Some("Cancelled".to_string()));
                return Ok(());
            }

            self.send_status(status_channel, exec_id, preset_id,
                ExecutionState::Running, 0, commands.len(), loop_count,
                Some(format!("Loop iteration {}", loop_count + 1)));

            // Execute all commands in this iteration
            for (i, cmd) in commands.iter().enumerate() {
                if self.check_cancelled(exec_id).await {
                    self.send_status(status_channel, exec_id, preset_id,
                        ExecutionState::Cancelled, i, commands.len(), loop_count, Some("Cancelled".to_string()));
                    return Ok(());
                }

                self.send_status(status_channel, exec_id, preset_id,
                    ExecutionState::Running, i, commands.len(), loop_count,
                    Some(format!("Loop {} [{}/{}]: {}", loop_count + 1, i + 1, commands.len(), cmd.command)));

                let write_result = manager.write_to(connection_id, &(cmd.command.clone() + "\n")).await;
                if write_result.is_err() && stop_on_error {
                    self.send_status(status_channel, exec_id, preset_id,
                        ExecutionState::Failed, i, commands.len(), loop_count,
                        Some(format!("Failed at loop {} command {}", loop_count + 1, i + 1)));
                    return Ok(());
                }

                if cmd.delay_ms > 0 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(cmd.delay_ms)).await;
                }
            }

            loop_count += 1;

            // Interval between loops
            if interval_ms > 0 && (count.is_none() || loop_count < max_loops) {
                tokio::time::sleep(tokio::time::Duration::from_millis(interval_ms)).await;
            }
        }

        self.send_status(status_channel, exec_id, preset_id,
            ExecutionState::Completed, commands.len(), commands.len(), loop_count, Some("Loop completed".to_string()));
        Ok(())
    }

    /// Send execution status to frontend
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
    ) {
        let status = PresetExecutionStatus {
            exec_id: exec_id.to_string(),
            preset_id: preset_id.to_string(),
            state,
            current_command_index,
            total_commands,
            current_loop,
            message,
        };
        if let Ok(json) = serde_json::to_string(&status) {
            let _ = channel.send(InvokeResponseBody::Json(json));
        }
    }

    /// Stop a running preset execution
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