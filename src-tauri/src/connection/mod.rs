pub mod manager;
pub mod ssh;
pub mod telnet;
pub mod local;

use serde::{Deserialize, Serialize};

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