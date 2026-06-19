pub mod manager;
pub mod ssh;
pub mod telnet;
pub mod local;

use std::sync::{Arc, Mutex as StdMutex};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

/// Output tap: an optional subscriber that receives a copy of every byte the
/// connection produces, in addition to the normal frontend (xterm) channel.
/// Used by the preset engine to capture per-command output for matching.
/// `None` means "no subscriber" — bytes only go to the frontend (the default).
pub type OutputTap = Arc<StdMutex<Option<UnboundedSender<Vec<u8>>>>>;

/// Create a fresh, empty tap (no subscriber).
pub fn new_output_tap() -> OutputTap {
    Arc::new(StdMutex::new(None))
}

/// Forward a copy of `bytes` to the tap's subscriber, if any. Never blocks:
/// unbounded channel; errors (subscriber dropped) are ignored.
pub fn tap_send(tap: &OutputTap, bytes: &[u8]) {
    if let Ok(guard) = tap.lock() {
        if let Some(sender) = guard.as_ref() {
            let _ = sender.send(bytes.to_vec());
        }
    }
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