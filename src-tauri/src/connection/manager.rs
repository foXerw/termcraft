use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use tauri::ipc::Channel;

use crate::connection::ssh::SSHHandler;
use crate::connection::telnet::TelnetHandler;
use crate::connection::local::LocalShellHandler;
use crate::connection::{ConnectionInfo, OutputTap};
use crate::errors::AppError;

/// Connection type enum for storing different handlers
enum ConnectionEntry {
    SSH(Arc<Mutex<SSHHandler>>),
    Telnet(Arc<Mutex<TelnetHandler>>),
    Local(Arc<Mutex<LocalShellHandler>>),
}

/// Manages all active connections
pub struct ConnectionManager {
    connections: Mutex<HashMap<String, ConnectionEntry>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
        }
    }

    /// Register an SSH connection
    pub async fn register_ssh(&self, id: String, handler: SSHHandler) {
        self.connections.lock().await.insert(id, ConnectionEntry::SSH(Arc::new(Mutex::new(handler))));
    }

    /// Register a Telnet connection
    pub async fn register_telnet(&self, id: String, handler: TelnetHandler) {
        self.connections.lock().await.insert(id, ConnectionEntry::Telnet(Arc::new(Mutex::new(handler))));
    }

    /// Register a local shell connection
    pub async fn register_local(&self, id: String, handler: LocalShellHandler) {
        self.connections.lock().await.insert(id, ConnectionEntry::Local(Arc::new(Mutex::new(handler))));
    }

    /// Remove a connection (and disconnect it)
    pub async fn remove(&self, id: &str) {
        let entry = self.connections.lock().await.remove(id);
        if let Some(entry) = entry {
            match entry {
                ConnectionEntry::SSH(h) => {
                    let mut handler = h.lock().await;
                    handler.disconnect().await.ok();
                }
                ConnectionEntry::Telnet(h) => {
                    let mut handler = h.lock().await;
                    handler.disconnect().await.ok();
                }
                ConnectionEntry::Local(h) => {
                    let mut handler = h.lock().await;
                    handler.disconnect().ok();
                }
            }
        }
    }

    /// List all connection infos
    pub async fn list(&self) -> Vec<ConnectionInfo> {
        let conns = self.connections.lock().await;
        let mut infos = Vec::new();
        for (id, entry) in conns.iter() {
            match entry {
                ConnectionEntry::SSH(h) => {
                    let c = h.lock().await;
                    infos.push(ConnectionInfo {
                        id: id.clone(),
                        name: String::new(),
                        conn_type: c.conn_type(),
                        alive: c.is_alive(),
                    });
                }
                ConnectionEntry::Telnet(h) => {
                    let c = h.lock().await;
                    infos.push(ConnectionInfo {
                        id: id.clone(),
                        name: String::new(),
                        conn_type: c.conn_type(),
                        alive: c.is_alive(),
                    });
                }
                ConnectionEntry::Local(h) => {
                    let c = h.lock().await;
                    infos.push(ConnectionInfo {
                        id: id.clone(),
                        name: String::new(),
                        conn_type: c.conn_type(),
                        alive: c.is_alive(),
                    });
                }
            }
        }
        infos
    }

    /// Write data to a specific connection
    pub async fn write_to(&self, id: &str, data: &str) -> Result<(), AppError> {
        let conns = self.connections.lock().await;
        let entry = conns.get(id)
            .ok_or(AppError::NotFound(format!("Connection {} not found", id)))?;

        match entry {
            ConnectionEntry::SSH(h) => {
                let c = h.lock().await;
                c.write(data).await
            }
            ConnectionEntry::Telnet(h) => {
                let c = h.lock().await;
                c.write(data).await
            }
            ConnectionEntry::Local(h) => {
                let c = h.lock().await;
                c.write(data)
            }
        }
    }

    /// Get the output tap of a connection (None if no such connection).
    async fn tap_of(&self, id: &str) -> Option<OutputTap> {
        let conns = self.connections.lock().await;
        let entry = conns.get(id)?;
        Some(match entry {
            ConnectionEntry::SSH(h) => h.lock().await.output_tap(),
            ConnectionEntry::Telnet(h) => h.lock().await.output_tap(),
            ConnectionEntry::Local(h) => h.lock().await.output_tap(),
        })
    }

    /// Subscribe to a connection's output stream. Returns a receiver that yields
    /// a copy of every byte the connection produces. Replaces any existing
    /// subscriber. Drop the receiver or call `unsubscribe_output` when done.
    pub async fn subscribe_output(&self, id: &str) -> Option<UnboundedReceiver<Vec<u8>>> {
        let tap = self.tap_of(id).await?;
        let (tx, rx) = unbounded_channel();
        if let Ok(mut guard) = tap.lock() {
            *guard = Some(tx);
        }
        Some(rx)
    }

    /// Remove the output subscriber from a connection (e.g. when a preset run ends).
    pub async fn unsubscribe_output(&self, id: &str) {
        if let Some(tap) = self.tap_of(id).await {
            if let Ok(mut guard) = tap.lock() {
                *guard = None;
            }
        }
    }

    /// Resize a specific connection
    pub async fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), AppError> {
        let conns = self.connections.lock().await;
        let entry = conns.get(id)
            .ok_or(AppError::NotFound(format!("Connection {} not found", id)))?;

        match entry {
            ConnectionEntry::SSH(h) => {
                let c = h.lock().await;
                c.resize(cols, rows).await
            }
            ConnectionEntry::Telnet(h) => {
                // Telnet doesn't have resize — it's a raw TCP stream
                Ok(())
            }
            ConnectionEntry::Local(h) => {
                let c = h.lock().await;
                c.resize(cols, rows)
            }
        }
    }
}