use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use tauri::ipc::Channel;

use crate::connection::ssh::SSHHandler;
use crate::connection::telnet::TelnetHandler;
use crate::connection::local::LocalShellHandler;
use crate::connection::logger::{LogChunk, LoggerHandle};
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
    loggers: Mutex<HashMap<String, LoggerHandle>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
            loggers: Mutex::new(HashMap::new()),
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

    /// Subscribe to a connection's output stream. Returns a `(sub_id, receiver)`
    /// pair; the receiver yields a copy of every byte the connection produces.
    /// Multiple subscribers may exist concurrently (preset engine + terminal
    /// logger). Remove a specific subscriber with `unsubscribe_output(id, sub_id)`.
    pub async fn subscribe_output(&self, id: &str) -> Option<(u64, UnboundedReceiver<Vec<u8>>)> {
        let tap = self.tap_of(id).await?;
        let (tx, rx) = unbounded_channel();
        let sub_id = {
            let mut guard = tap.lock().ok()?;
            let id = guard.next_sub_id;
            guard.next_sub_id += 1;
            guard.senders.push((id, tx));
            id
        };
        Some((sub_id, rx))
    }

    /// Remove a specific output subscriber (by `sub_id`) from a connection.
    pub async fn unsubscribe_output(&self, id: &str, sub_id: u64) {
        if let Some(tap) = self.tap_of(id).await {
            if let Ok(mut guard) = tap.lock() {
                guard.senders.retain(|(sid, _)| *sid != sub_id);
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

    /// Start logging a connection's output+input to `path`. Fails if the
    /// connection doesn't exist or is already being logged.
    pub async fn start_logging(&self, id: &str, path: String) -> Result<(), AppError> {
        {
            let loggers = self.loggers.lock().await;
            if loggers.contains_key(id) {
                return Err(AppError::Connection("该终端已在记录日志".to_string()));
            }
        }
        let (sub_id, rx_out) = match self.subscribe_output(id).await {
            Some(pair) => pair,
            None => {
                return Err(AppError::NotFound(format!(
                    "Connection {} not found",
                    id
                )))
            }
        };
        let handle = LoggerHandle::start(&path, rx_out, sub_id)?;
        self.loggers.lock().await.insert(id.to_string(), handle);
        Ok(())
    }

    /// Stop logging a connection (idempotent). Drops the writer sender (ends the
    /// writer task, closing the file) and detaches the output-tap subscriber.
    pub async fn stop_logging(&self, id: &str) {
        let handle = self.loggers.lock().await.remove(id);
        if let Some(handle) = handle {
            self.unsubscribe_output(id, handle.out_sub_id).await;
        }
    }

    /// Forward user input bytes to a connection's logger, if one is active.
    /// No-op (not an error) when the connection isn't being logged.
    pub async fn log_input(&self, id: &str, data: &[u8]) {
        let loggers = self.loggers.lock().await;
        if let Some(handle) = loggers.get(id) {
            let _ = handle.sender.send(LogChunk::Input(data.to_vec()));
        }
    }
}