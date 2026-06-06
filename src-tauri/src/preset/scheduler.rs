use std::collections::HashMap;

use crate::preset::models::ScheduledTask;

/// Simple scheduler using tokio timers
/// For Cron scheduling, a proper cron parser would be needed (e.g., cron crate)
pub struct PresetScheduler {
    active_tasks: HashMap<String, tokio::task::JoinHandle<()>>,
}

impl PresetScheduler {
    pub fn new() -> Self {
        Self {
            active_tasks: HashMap::new(),
        }
    }

    /// Start a scheduled task (Interval mode only for now)
    pub fn start_interval(
        &mut self,
        _task: ScheduledTask,
        interval_secs: u64,
    ) {
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(interval_secs)
            );
            loop {
                interval.tick().await;
                // TODO: integrate with PresetEngine for actual execution
            }
        });

        self.active_tasks.insert(String::new(), handle);
    }

    /// Stop a scheduled task
    pub fn stop(&mut self, task_id: &str) {
        if let Some(handle) = self.active_tasks.remove(task_id) {
            handle.abort();
        }
    }

    /// Stop all scheduled tasks
    pub fn stop_all(&mut self) {
        for (_, handle) in self.active_tasks.drain() {
            handle.abort();
        }
    }
}