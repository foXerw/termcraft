pub mod manager;
pub mod ssh;
pub mod telnet;
pub mod local;
pub mod logger;

use std::sync::{Arc, Mutex as StdMutex};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

/// Output tap: a set of subscribers, each receiving a copy of every byte the
/// connection produces (in addition to the normal frontend/xterm channel).
/// Multi-subscriber so the preset engine and terminal logging can both listen
/// without one clobbering the other. Each subscriber is identified by a
/// monotonically-increasing `sub_id` returned from `subscribe_output` (in the
/// manager) and removed via `unsubscribe_output`.
pub struct OutputTapInner {
    pub senders: Vec<(u64, UnboundedSender<Vec<u8>>)>,
    pub next_sub_id: u64,
}

pub type OutputTap = Arc<StdMutex<OutputTapInner>>;

/// Create a fresh, empty tap (no subscribers).
pub fn new_output_tap() -> OutputTap {
    Arc::new(StdMutex::new(OutputTapInner {
        senders: Vec::new(),
        next_sub_id: 0,
    }))
}

/// Forward a copy of `bytes` to every tap subscriber. Never blocks: unbounded
/// channels; subscribers whose receiver has been dropped are pruned.
pub fn tap_send(tap: &OutputTap, bytes: &[u8]) {
    if let Ok(mut guard) = tap.lock() {
        guard.senders.retain(|(_, sender)| sender.send(bytes.to_vec()).is_ok());
    }
}

/// Event name emitted when a connection's stream ends (EOF / shell exited /
/// disconnected). Frontend removes the corresponding tab (or respawns the
/// default local terminal) on receipt.
pub const CLOSED_EVENT: &str = "connection_closed";

/// Emit [`CLOSED_EVENT`] for `id`. Used by handlers when their read loop ends.
pub fn emit_closed(app: &tauri::AppHandle, id: &str) {
    use tauri::Emitter;
    let _ = app.emit(CLOSED_EVENT, id.to_string());
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum ConnType {
    SSH,
    Telnet,
    LocalShell,
}

/// Authentication config for SSH connections
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum AuthConfig {
    Password { password: String },
    PublicKey { key_path: String, passphrase: Option<String> },
    Agent,
}

/// Connection config for saving/loading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub id: String,
    pub name: String,
    pub conn_type: ConnType,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub auth: Option<AuthConfig>,
    pub shell: Option<String>,
    pub tags: Vec<String>,
}

/// Runtime connection info (not persisted, just status)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub id: String,
    pub name: String,
    pub conn_type: ConnType,
    pub alive: bool,
}