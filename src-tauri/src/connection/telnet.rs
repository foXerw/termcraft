use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use crate::connection::ConnType;
use crate::errors::AppError;

type WriteHalf = tokio::io::WriteHalf<TcpStream>;

pub struct TelnetHandler {
    id: String,
    host: String,
    port: u16,
    write_half: Option<Arc<Mutex<WriteHalf>>>,
    alive: bool,
    read_task: Option<tokio::task::JoinHandle<()>>,
}

impl TelnetHandler {
    pub fn new(id: String, host: String, port: u16) -> Self {
        Self {
            id,
            host,
            port,
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
        let read_task = tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            use tokio::io::AsyncReadExt;
            let mut reader = read_half;
            loop {
                match reader.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let text = String::from_utf8_lossy(&buf[..n]);
                        let json = serde_json::to_string(&text).unwrap_or_default();
                        let _ = frontend_channel.send(tauri::ipc::InvokeResponseBody::Json(json));
                    }
                    Err(_) => break,
                }
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

    pub fn is_alive(&self) -> bool {
        self.alive
    }

    pub fn conn_type(&self) -> ConnType {
        ConnType::Telnet
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}