pub mod manager;
pub mod ssh;
pub mod telnet;
pub mod local;
pub mod serial;
pub mod logger;

use std::sync::{Arc, Mutex as StdMutex};

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tokio::sync::mpsc::UnboundedSender;

use crate::errors::AppError;

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
    Serial,
}

/// Serial line framing parameters for `ConnType::Serial` connections.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum SerialDataBits {
    Five,
    Six,
    Seven,
    Eight,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum SerialParity {
    None,
    Odd,
    Even,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum SerialStopBits {
    One,
    Two,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum SerialFlowControl {
    None,
    Software,
}

/// Full serial configuration. Stored nested under `ConnectionConfig::serial`
/// (which is `Option<SerialConfig>` so old SSH/Telnet/LocalShell JSON stays
/// loadable).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SerialConfig {
    /// Device path, e.g. `COM3` on Windows or `/dev/ttyUSB0` on Linux.
    pub port_path: String,
    pub baud_rate: u32,
    pub data_bits: SerialDataBits,
    pub parity: SerialParity,
    pub stop_bits: SerialStopBits,
    pub flow_control: SerialFlowControl,
}

impl Default for SerialConfig {
    /// 9600 8N1, no flow control — the conservative serial default. `port_path`
    /// is blank; real configs always set it from the port picker.
    fn default() -> Self {
        Self {
            port_path: String::new(),
            baud_rate: 9600,
            data_bits: SerialDataBits::Eight,
            parity: SerialParity::None,
            stop_bits: SerialStopBits::One,
            flow_control: SerialFlowControl::None,
        }
    }
}

/// Uniform interface every connection handler (SSH / Telnet / LocalShell /
/// Serial) implements, so `ConnectionManager` can store and dispatch over
/// `Box<dyn ConnHandler + Send + Sync>` instead of an enum with a per-variant
/// match arm in every method. Adding a new protocol = one new `impl` block,
/// no manager changes.
#[async_trait::async_trait]
pub trait ConnHandler: Send + Sync {
    fn id(&self) -> &str;
    fn conn_type(&self) -> ConnType;
    /// Human-readable name for the connection (used by `list_connections`).
    fn name(&self) -> &str;
    fn is_alive(&self) -> bool;
    fn output_tap(&self) -> OutputTap;

    /// Establish the connection and start forwarding output to `channel`.
    async fn connect(&mut self, channel: Channel) -> Result<(), AppError>;
    async fn write(&self, data: &str) -> Result<(), AppError>;
    async fn disconnect(&mut self) -> Result<(), AppError>;
    /// Resize the terminal window. Default no-op for protocols without a PTY
    /// concept (Telnet, Serial).
    async fn resize(&self, _cols: u16, _rows: u16) -> Result<(), AppError> {
        Ok(())
    }
}

/// Encode raw terminal bytes as a JSON string and forward them to the frontend
/// channel. Shared by every handler so the read loops stay in sync. Terminal
/// data may contain ANSI escapes and invalid UTF-8, hence `from_utf8_lossy`.
pub fn forward_to_frontend(channel: &Channel, bytes: &[u8]) {
    let text = String::from_utf8_lossy(bytes);
    let json = serde_json::to_string(&text).unwrap_or_default();
    let _ = channel.send(tauri::ipc::InvokeResponseBody::Json(json));
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
    /// Present only for `ConnType::Serial` connections.
    pub serial: Option<SerialConfig>,
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