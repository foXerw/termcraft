//! Connection reachability probing.
//!
//! A single background tokio task probes each registered connection's
//! `host:port` with a short-timeout TCP connect and pushes status changes to
//! the frontend via the `connection_status` event. Probing is staggered and
//! backs off on failure so a down host is never hammered.
//!
//! See `docs/superpowers/specs/2026-06-19-connection-reachability-design.md`.

use std::collections::HashMap;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

/// Base (success) interval between probes, in seconds.
const BASE_INTERVAL_SECS: u64 = 10;
/// Max backoff interval on repeated failure, in seconds.
const CAP_SECS: u64 = 20;
/// Per-probe connect timeout.
const PROBE_TIMEOUT: Duration = Duration::from_millis(1500);
/// Scheduler wake-up granularity.
const TICK: Duration = Duration::from_secs(1);
/// Stagger between targets so they don't all fire at once.
const STAGGER_SECS: u64 = 2;
/// After an event-driven Down (an active session dropped), re-probe this soon
/// so recovery is caught quickly and any false-positive Down self-corrects.
const REPROBE_AFTER_DOWN_SECS: u64 = 3;

/// Event name pushed to the frontend on status change.
pub const EVENT: &str = "connection_status";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Checking,
    Reachable,
    Down,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
struct StatusPayload {
    id: String,
    status: Status,
    latency_ms: Option<u32>,
    last_checked: Option<String>,
}

/// Per-target scheduling + last-known state.
struct Target {
    host: String,
    port: u16,
    /// Current interval between probes, grows on failure.
    interval_secs: u64,
    /// Next probe time, as epoch millis.
    next_check_at: i64,
    status: Status,
    latency_ms: Option<u32>,
    last_checked: Option<String>,
}

pub struct ReachabilityService {
    targets: Mutex<HashMap<String, Target>>,
}

impl ReachabilityService {
    pub fn new() -> Self {
        Self {
            targets: Mutex::new(HashMap::new()),
        }
    }

    /// Replace the watched target set. Newly added targets probe ASAP (staggered
    /// by insertion order); removed targets are dropped. Existing targets keep
    /// their backoff state so editing a host doesn't reset unrelated hosts.
    pub async fn set_targets(&self, entries: Vec<(String, String, u16)>) {
        let mut targets = self.targets.lock().await;
        let now = Utc::now().timestamp_millis();
        let mut keep = std::collections::HashSet::new();

        for (i, (id, host, port)) in entries.into_iter().enumerate() {
            keep.insert(id.clone());
            targets.entry(id).and_modify(|t| {
                t.host = host.clone();
                t.port = port;
            }).or_insert(Target {
                host,
                port,
                interval_secs: BASE_INTERVAL_SECS,
                // Stagger first probe by insertion order so a freshly-loaded
                // list doesn't fire everything in the same tick.
                next_check_at: now + i as i64 * (STAGGER_SECS as i64) * 1000,
                status: Status::Checking,
                latency_ms: None,
                last_checked: None,
            });
        }
        // Drop targets no longer present (deleted connections).
        targets.retain(|id, _| keep.contains(id));
    }

    /// Event-driven Down: an active SSH/Telnet session to `host:port` just
    /// dropped (transport disconnect / network reset — e.g. the device is
    /// rebooting). Immediately mark every target matching that endpoint Down
    /// and schedule a near-term re-probe, so:
    ///   - the status dot flips red without waiting for the next poll tick;
    ///   - recovery is caught within ~`REPROBE_AFTER_DOWN_SECS` + one poll;
    ///   - a false positive (e.g. a clean disconnect we misclassified)
    ///     self-corrects to green on the re-probe.
    ///
    /// Matching is by `host:port`, not by connection id, because a reconnected
    /// tab gets a fresh id distinct from the saved config's id.
    pub async fn mark_down_by_endpoint(&self, app: &AppHandle, host: &str, port: u16) {
        let now = Utc::now().timestamp_millis();
        let mut targets = self.targets.lock().await;
        for (id, t) in targets.iter_mut() {
            if t.host != host || t.port != port || t.status == Status::Down {
                continue;
            }
            t.status = Status::Down;
            t.latency_ms = None;
            t.last_checked = Some(Utc::now().to_rfc3339());
            t.next_check_at = now + (REPROBE_AFTER_DOWN_SECS as i64) * 1000;
            let _ = app.emit(
                EVENT,
                StatusPayload {
                    id: id.clone(),
                    status: Status::Down,
                    latency_ms: None,
                    last_checked: t.last_checked.clone(),
                },
            );
        }
    }

    /// Spawn the single scheduler loop. Owns its own copy of the Arc + the
    /// AppHandle used to emit events.
    pub fn spawn(self: Arc<Self>, app: AppHandle) {
        // Use Tauri's async runtime so the task runs within the tokio reactor;
        // calling tokio::spawn directly from the setup hook panics because no
        // reactor is registered on that thread.
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(TICK).await;
                Self::tick(&app, &self).await;
            }
        });
    }

    /// One scheduler pass: collect due targets (snapshot, release lock), probe
    /// each, then write results back and emit any status change.
    async fn tick(app: &AppHandle, svc: &Arc<ReachabilityService>) {
        let now = Utc::now().timestamp_millis();

        // Snapshot of targets due now and the data needed to probe them.
        let due: Vec<(String, String, u16)> = {
            let targets = svc.targets.lock().await;
            targets
                .iter()
                .filter(|(_, t)| t.next_check_at <= now)
                .map(|(id, t)| (id.clone(), t.host.clone(), t.port))
                .collect()
        };

        for (id, host, port) in due {
            let result = probe(&host, port).await;
            let mut targets = svc.targets.lock().await;
            let Some(t) = targets.get_mut(&id) else { continue };

            let now_ts = Utc::now();
            let (new_status, new_latency, next_interval) = match result {
                Ok(ms) => (Status::Reachable, Some(ms), BASE_INTERVAL_SECS),
                Err(()) => {
                    // Exponential backoff on failure, capped.
                    let next = (t.interval_secs.saturating_mul(2)).min(CAP_SECS);
                    (Status::Down, None, next)
                }
            };

            let changed = new_status != t.status;
            t.status = new_status;
            t.latency_ms = new_latency;
            t.last_checked = Some(now_ts.to_rfc3339());
            t.interval_secs = next_interval;
            t.next_check_at = Utc::now().timestamp_millis() + (next_interval as i64 * 1000);

            if changed {
                let _ = app.emit(
                    EVENT,
                    StatusPayload {
                        id: id.clone(),
                        status: new_status,
                        latency_ms: new_latency,
                        last_checked: t.last_checked.clone(),
                    },
                );
            }
        }
    }
}

/// Probe one endpoint: connect with a short timeout, return latency on success.
async fn probe(host: &str, port: u16) -> Result<u32, ()> {
    // Resolve once up front so the timeout covers DNS too.
    let addr = match (host, port).to_socket_addrs() {
        Ok(mut it) => match it.next() {
            Some(a) => a,
            None => return Err(()),
        },
        Err(_) => return Err(()),
    };
    let start = Instant::now();
    let conn = tokio::time::timeout(PROBE_TIMEOUT, TcpStream::connect(&addr)).await;
    match conn {
        Ok(Ok(_)) => Ok(start.elapsed().as_millis() as u32),
        _ => Err(()),
    }
}

/// Convenience accessor for AppState.
pub fn init() -> Arc<ReachabilityService> {
    Arc::new(ReachabilityService::new())
}
