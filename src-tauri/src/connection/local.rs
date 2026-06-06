use std::sync::{Arc, Mutex as StdMutex};

use portable_pty::{native_pty_system, CommandBuilder, PtySize, MasterPty, Child};

use crate::connection::ConnType;
use crate::errors::AppError;

pub struct LocalShellHandler {
    id: String,
    shell: String,
    master: Option<Arc<StdMutex<Box<dyn MasterPty + Send>>>>,
    writer: Option<Arc<StdMutex<Box<dyn std::io::Write + Send>>>>,
    child: Option<Box<dyn Child + Send + Sync>>,
    alive: bool,
    read_task: Option<tokio::task::JoinHandle<()>>,
}

impl LocalShellHandler {
    pub fn new(id: String, shell: String) -> Self {
        Self {
            id,
            shell,
            master: None,
            writer: None,
            child: None,
            alive: false,
            read_task: None,
        }
    }

    /// Start a local shell PTY
    pub fn connect(&mut self, frontend_channel: tauri::ipc::Channel) -> Result<(), AppError> {
        let pty_system = native_pty_system();

        let size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system
            .openpty(size)
            .map_err(|e| AppError::LocalShell(format!("Failed to open PTY: {}", e)))?;

        let shell_cmd = if self.shell.is_empty() {
            if std::env::var("SHELL").is_ok() {
                std::env::var("SHELL").unwrap()
            } else {
                "cmd.exe".to_string()
            }
        } else {
            self.shell.clone()
        };

        let cmd = CommandBuilder::new(shell_cmd);
        let child = pair.slave
            .spawn_command(cmd)
            .map_err(|e| AppError::LocalShell(format!("Failed to spawn shell: {}", e)))?;

        // Drop the slave side
        drop(pair.slave);

        // Get writer from master — take_writer gives us a Box<dyn Write + Send>
        let writer = pair.master.take_writer()
            .map_err(|e| AppError::LocalShell(format!("Failed to take writer: {}", e)))?;

        let master = Arc::new(StdMutex::new(pair.master));
        let writer = Arc::new(StdMutex::new(writer));
        self.master = Some(master.clone());
        self.writer = Some(writer);
        self.child = Some(child);
        self.alive = true;

        // Start reading task
        let read_task = tokio::spawn(async move {
            let reader = {
                let m = master.lock().unwrap();
                m.try_clone_reader().ok()
            };

            if let Some(mut reader) = reader {
                tokio::task::spawn_blocking(move || {
                    let mut buf = [0u8; 4096];
                    use std::io::Read;
                    loop {
                        match reader.read(&mut buf) {
                            Ok(0) => break,
                            Ok(n) => {
                                let text = String::from_utf8_lossy(&buf[..n]);
                                let json = serde_json::to_string(&text).unwrap_or_default();
                                let _ = frontend_channel.send(tauri::ipc::InvokeResponseBody::Json(json));
                            }
                            Err(_) => break,
                        }
                    }
                }).await.ok();
            }
        });

        self.read_task = Some(read_task);
        Ok(())
    }

    /// Write data to the PTY
    pub fn write(&self, data: &str) -> Result<(), AppError> {
        if !self.alive || self.writer.is_none() {
            return Err(AppError::Connection("Local shell is not alive".to_string()));
        }

        let writer = self.writer.as_ref().unwrap();
        let mut w = writer.lock().unwrap();
        w.write_all(data.as_bytes())
            .map_err(|e| AppError::LocalShell(format!("Failed to write: {}", e)))?;
        Ok(())
    }

    /// Resize the PTY
    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), AppError> {
        if !self.alive || self.master.is_none() {
            return Err(AppError::Connection("Local shell is not alive".to_string()));
        }

        let master = self.master.as_ref().unwrap();
        let m = master.lock().unwrap();
        m.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| AppError::LocalShell(format!("Failed to resize: {}", e)))?;
        Ok(())
    }

    /// Disconnect (close PTY)
    pub fn disconnect(&mut self) -> Result<(), AppError> {
        self.alive = false;
        if let Some(task) = self.read_task.take() {
            task.abort();
        }
        self.master = None;
        self.writer = None;
        if let Some(mut child) = self.child.take() {
            child.kill().ok();
        }
        Ok(())
    }

    pub fn is_alive(&self) -> bool {
        self.alive
    }

    pub fn conn_type(&self) -> ConnType {
        ConnType::LocalShell
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}