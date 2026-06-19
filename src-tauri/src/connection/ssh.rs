use std::sync::Arc;
use tokio::sync::mpsc;

use russh::*;
use russh::client::{Handle, Handler, Session};
use russh_keys::key::PublicKey as SshPublicKey;

use crate::connection::{AuthConfig, ConnType, OutputTap, new_output_tap, tap_send};
use crate::errors::AppError;
use tauri::ipc::InvokeResponseBody;

/// Custom client handler that forwards received data to an mpsc channel
struct SSHClientHandler {
    output_tx: mpsc::UnboundedSender<Vec<u8>>,
}

#[async_trait::async_trait]
impl Handler for SSHClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &SshPublicKey,
    ) -> Result<bool, Self::Error> {
        // TODO: implement proper server key verification / known_hosts
        Ok(true)
    }

    async fn channel_open_confirmation(
        &mut self,
        id: ChannelId,
        max_packet_size: u32,
        window_size: u32,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        // Forward data to the output channel
        let _ = self.output_tx.send(data.to_vec());
        Ok(())
    }

    async fn extended_data(
        &mut self,
        channel: ChannelId,
        ext: u32,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        // Extended data (e.g., stderr) — forward as well
        let _ = self.output_tx.send(data.to_vec());
        Ok(())
    }
}

pub struct SSHHandler {
    id: String,
    host: String,
    port: u16,
    username: String,
    auth: AuthConfig,
    output_tap: OutputTap,
    session_handle: Option<Handle<SSHClientHandler>>,
    channel: Option<Channel<client::Msg>>,
    alive: bool,
    output_rx: Option<mpsc::UnboundedReceiver<Vec<u8>>>,
    forward_task: Option<tokio::task::JoinHandle<()>>,
}

impl SSHHandler {
    pub fn new(id: String, host: String, port: u16, username: String, auth: AuthConfig) -> Self {
        Self {
            id,
            host,
            port,
            username,
            auth,
            output_tap: new_output_tap(),
            session_handle: None,
            channel: None,
            alive: false,
            output_rx: None,
            forward_task: None,
        }
    }

    pub fn output_tap(&self) -> OutputTap {
        self.output_tap.clone()
    }

    /// Connect to SSH server asynchronously
    pub async fn connect(&mut self) -> Result<(), AppError> {
        let (output_tx, output_rx) = mpsc::unbounded_channel();

        let config = client::Config::default();
        let handler = SSHClientHandler { output_tx };

        let mut session_handle = client::connect(
            Arc::new(config),
            (self.host.as_str(), self.port),
            handler,
        )
        .await
        .map_err(|e| AppError::Ssh(format!("Failed to connect to {}:{}", self.host, self.port)))?;

        // Authenticate
        match &self.auth {
            AuthConfig::Password { password } => {
                let auth_result = session_handle
                    .authenticate_password(self.username.as_str(), password.as_str())
                    .await
                    .map_err(|e| AppError::Ssh(format!("Authentication error: {}", e)))?;
                if !auth_result {
                    return Err(AppError::Ssh("Password authentication failed".to_string()));
                }
            }
            AuthConfig::PublicKey { key_path, passphrase } => {
                let key_pair = russh_keys::load_secret_key(key_path, passphrase.as_deref())
                    .map_err(|e| AppError::Ssh(format!("Failed to load key {}: {}", key_path, e)))?;
                let auth_result = session_handle
                    .authenticate_publickey(self.username.as_str(), Arc::new(key_pair))
                    .await
                    .map_err(|e| AppError::Ssh(format!("Key auth error: {}", e)))?;
                if !auth_result {
                    return Err(AppError::Ssh("Public key authentication failed".to_string()));
                }
            }
            AuthConfig::Agent => {
                return Err(AppError::Ssh("SSH agent auth not yet implemented".to_string()));
            }
        }

        // Open session channel
        let channel = session_handle
            .channel_open_session()
            .await
            .map_err(|e| AppError::Ssh(format!("Failed to open channel: {}", e)))?;

        // Request PTY with xterm-256color, default 80x24
        // request_pty(want_reply, term, col_width, row_height, pix_width, pix_height, terminal_modes)
        channel
            .request_pty(
                true,                    // want_reply
                "xterm-256color",        // term
                80,                      // col_width
                24,                      // row_height
                0,                       // pix_width
                0,                       // pix_height
                &[],                     // terminal_modes
            )
            .await
            .map_err(|e| AppError::Ssh(format!("Failed to request PTY: {}", e)))?;

        // Request shell
        channel
            .request_shell(true)
            .await
            .map_err(|e| AppError::Ssh(format!("Failed to request shell: {}", e)))?;

        self.channel = Some(channel);
        self.session_handle = Some(session_handle);
        self.output_rx = Some(output_rx);
        self.alive = true;

        Ok(())
    }

    /// Start forwarding data from SSH output to frontend
    pub fn start_forward_task(
        &mut self,
        frontend_channel: tauri::ipc::Channel,
    ) {
        if let Some(mut output_rx) = self.output_rx.take() {
            let tap = self.output_tap.clone();
            let task = tokio::spawn(async move {
                while let Some(data) = output_rx.recv().await {
                    // Copy raw bytes to any output subscriber (preset engine) first.
                    tap_send(&tap, &data);
                    // Convert raw bytes to string — terminal data may contain
                    // escape sequences, so we need to handle UTF-8 properly
                    let text = String::from_utf8_lossy(&data);
                    // Wrap the terminal text as a JSON string value so the frontend
                    // can deserialize it correctly. Terminal data contains ANSI
                    // escape sequences which must survive JSON serialization.
                    let json_str = serde_json::to_string(&text).unwrap_or_default();
                    let _ = frontend_channel.send(InvokeResponseBody::Json(json_str));
                }
            });
            self.forward_task = Some(task);
        }
    }

    /// Disconnect from SSH server
    pub async fn disconnect(&mut self) -> Result<(), AppError> {
        self.alive = false;

        // Stop the forward task
        if let Some(task) = self.forward_task.take() {
            task.abort();
        }

        // Disconnect the session
        if let Some(handle) = self.session_handle.take() {
            handle
                .disconnect(Disconnect::ByApplication, "Disconnecting", "")
                .await
                .map_err(|e| AppError::Ssh(format!("Disconnect error: {}", e)))?;
        }

        self.channel = None;
        Ok(())
    }

    /// Write data to the SSH channel
    pub async fn write(&self, data: &str) -> Result<(), AppError> {
        if !self.alive || self.channel.is_none() {
            return Err(AppError::Connection("SSH connection is not alive".to_string()));
        }

        let channel = self.channel.as_ref().unwrap();
        channel
            .data(data.as_bytes())
            .await
            .map_err(|_| AppError::Ssh("Failed to write data".to_string()))?;

        Ok(())
    }

    /// Resize the PTY
    pub async fn resize(&self, cols: u16, rows: u16) -> Result<(), AppError> {
        if !self.alive || self.channel.is_none() {
            return Err(AppError::Connection("SSH connection is not alive".to_string()));
        }

        let channel = self.channel.as_ref().unwrap();
        channel
            .window_change(cols as u32, rows as u32, 0, 0)
            .await
            .map_err(|e| AppError::Ssh(format!("Failed to resize PTY: {}", e)))?;

        Ok(())
    }

    pub fn is_alive(&self) -> bool {
        self.alive
    }

    pub fn conn_type(&self) -> ConnType {
        ConnType::SSH
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}