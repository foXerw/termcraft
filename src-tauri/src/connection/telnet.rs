use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use crate::connection::{ConnType, ConnHandler, OutputTap, emit_closed, forward_to_frontend, new_output_tap, tap_send};
use crate::errors::AppError;
use crate::reachability::ReachabilityService;
use tauri::AppHandle;

type WriteHalf = tokio::io::WriteHalf<TcpStream>;

pub struct TelnetHandler {
    id: String,
    name: String,
    host: String,
    port: u16,
    output_tap: OutputTap,
    app: AppHandle,
    reachability: Arc<ReachabilityService>,
    write_half: Option<Arc<Mutex<WriteHalf>>>,
    alive: bool,
    read_task: Option<tokio::task::JoinHandle<()>>,
}

impl TelnetHandler {
    pub fn new(
        id: String,
        name: String,
        host: String,
        port: u16,
        app: AppHandle,
        reachability: Arc<ReachabilityService>,
    ) -> Self {
        Self {
            id,
            name,
            host,
            port,
            output_tap: new_output_tap(),
            app,
            reachability,
            write_half: None,
            alive: false,
            read_task: None,
        }
    }

    /// Connect to Telnet server asynchronously
    pub async fn connect(
        &mut self,
        frontend_channel: tauri::ipc::Channel,
    ) -> Result<(), AppError> {
        // Start reading task — use tokio::io::split to avoid lock contention
        let (read_half, write_half) = {
            let stream = TcpStream::connect((self.host.as_str(), self.port))
                .await
                .map_err(|e| AppError::Telnet(format!("Failed to connect to {}:{}", self.host, self.port)))?;
            let (r, w) = tokio::io::split(stream);
            (r, Arc::new(Mutex::new(w)))
        };
        self.write_half = Some(write_half);
        self.alive = true;

        // Start reading task
        let tap = self.output_tap.clone();
        let app = self.app.clone();
        let id = self.id.clone();
        let reachability = self.reachability.clone();
        let host = self.host.clone();
        let port = self.port;
        let read_task = tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            use tokio::io::AsyncReadExt;
            let mut reader = read_half;
            // Distinguish a network-level drop (read Err — connection reset /
            // aborted, e.g. device rebooting) from a clean close (Ok(0) —
            // server/logout FIN). Only the former flips reachability Down: a
            // clean logout is not a down host. False positives self-correct on
            // the scheduled re-probe.
            let mut errored = false;
            loop {
                match reader.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        tap_send(&tap, &buf[..n]);
                        forward_to_frontend(&frontend_channel, &buf[..n]);
                    }
                    Err(_) => {
                        errored = true;
                        break;
                    }
                }
            }
            // Connection ended (EOF / closed): notify the frontend.
            emit_closed(&app, &id);
            if errored {
                reachability.mark_down_by_endpoint(&app, &host, port).await;
            }
        });

        self.read_task = Some(read_task);
        Ok(())
    }

    /// Write data to the Telnet connection
    pub async fn write(&self, data: &str) -> Result<(), AppError> {
        if !self.alive || self.write_half.is_none() {
            return Err(AppError::Connection("Telnet connection is not alive".to_string()));
        }

        let write_half = self.write_half.as_ref().unwrap();
        let mut w = write_half.lock().await;
        w.write_all(data.as_bytes())
            .await
            .map_err(|e| AppError::Telnet(format!("Failed to write: {}", e)))?;
        Ok(())
    }

    /// Disconnect
    pub async fn disconnect(&mut self) -> Result<(), AppError> {
        self.alive = false;
        if let Some(task) = self.read_task.take() {
            task.abort();
        }
        self.write_half = None;
        Ok(())
    }
}

#[async_trait::async_trait]
impl ConnHandler for TelnetHandler {
    fn id(&self) -> &str {
        &self.id
    }
    fn conn_type(&self) -> ConnType {
        ConnType::Telnet
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn is_alive(&self) -> bool {
        self.alive
    }
    fn output_tap(&self) -> OutputTap {
        self.output_tap.clone()
    }
    async fn connect(&mut self, channel: tauri::ipc::Channel) -> Result<(), AppError> {
        TelnetHandler::connect(self, channel).await
    }
    async fn write(&self, data: &str) -> Result<(), AppError> {
        TelnetHandler::write(self, data).await
    }
    async fn disconnect(&mut self) -> Result<(), AppError> {
        TelnetHandler::disconnect(self).await
    }
    // resize: default no-op (raw TCP stream, no PTY).
}