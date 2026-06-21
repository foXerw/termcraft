use std::sync::{Arc, Mutex as StdMutex};

use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};

use crate::connection::{
    ConnHandler, ConnType, OutputTap, SerialConfig, SerialDataBits, SerialFlowControl,
    SerialParity, SerialStopBits, emit_closed, forward_to_frontend, new_output_tap, tap_send,
};
use crate::errors::AppError;
use tauri::AppHandle;

fn map_data_bits(d: SerialDataBits) -> DataBits {
    match d {
        SerialDataBits::Five => DataBits::Five,
        SerialDataBits::Six => DataBits::Six,
        SerialDataBits::Seven => DataBits::Seven,
        SerialDataBits::Eight => DataBits::Eight,
    }
}

fn map_parity(p: SerialParity) -> Parity {
    match p {
        SerialParity::None => Parity::None,
        SerialParity::Odd => Parity::Odd,
        SerialParity::Even => Parity::Even,
    }
}

fn map_stop_bits(s: SerialStopBits) -> StopBits {
    match s {
        SerialStopBits::One => StopBits::One,
        SerialStopBits::Two => StopBits::Two,
    }
}

fn map_flow_control(f: SerialFlowControl) -> FlowControl {
    match f {
        SerialFlowControl::None => FlowControl::None,
        SerialFlowControl::Software => FlowControl::Software,
    }
}

pub struct SerialHandler {
    id: String,
    name: String,
    port_path: String,
    config: SerialConfig,
    output_tap: OutputTap,
    app: AppHandle,
    /// The write side of the serial port. A clone (`try_clone`) feeds the
    /// blocking read loop in `spawn_blocking`, mirroring how the local PTY
    /// handler splits master/writer.
    port: Option<Arc<StdMutex<Box<dyn SerialPort>>>>,
    alive: bool,
    read_task: Option<tokio::task::JoinHandle<()>>,
}

impl SerialHandler {
    pub fn new(id: String, name: String, config: SerialConfig, app: AppHandle) -> Self {
        Self {
            id,
            name,
            // `port_path` lives inside `SerialConfig` so saved configs round-trip.
            port_path: config.port_path.clone(),
            output_tap: new_output_tap(),
            app,
            config,
            port: None,
            alive: false,
            read_task: None,
        }
    }

    /// Open the serial port and start forwarding output to the frontend.
    pub fn connect(&mut self, frontend_channel: tauri::ipc::Channel) -> Result<(), AppError> {
        let port = serialport::new(self.port_path.as_str(), self.config.baud_rate)
            .data_bits(map_data_bits(self.config.data_bits))
            .parity(map_parity(self.config.parity))
            .stop_bits(map_stop_bits(self.config.stop_bits))
            .flow_control(map_flow_control(self.config.flow_control))
            .open()
            .map_err(|e| AppError::Serial(format!("打开串口 {} 失败: {}", self.port_path, e)))?;

        // Clone the handle for the reader so reads and writes can happen
        // concurrently without sharing a lock on every byte.
        let reader = port
            .try_clone()
            .map_err(|e| AppError::Serial(format!("克隆串口失败: {}", e)))?;

        let port = Arc::new(StdMutex::new(port));
        self.port = Some(port);
        self.alive = true;

        // Read loop — blocking std::io::Read on a dedicated blocking thread,
        // same shape as LocalShellHandler.
        let tap = self.output_tap.clone();
        let app = self.app.clone();
        let id = self.id.clone();
        let read_task = tokio::spawn(async move {
            tokio::task::spawn_blocking(move || {
                let mut reader = reader;
                let mut buf = [0u8; 4096];
                use std::io::Read;
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            tap_send(&tap, &buf[..n]);
                            forward_to_frontend(&frontend_channel, &buf[..n]);
                        }
                        Err(_) => break,
                    }
                }
            })
            .await
            .ok();
            // Port closed / device gone: notify the frontend.
            emit_closed(&app, &id);
        });

        self.read_task = Some(read_task);
        Ok(())
    }

    pub fn write(&self, data: &str) -> Result<(), AppError> {
        if !self.alive || self.port.is_none() {
            return Err(AppError::Connection("Serial connection is not alive".to_string()));
        }
        let port = self.port.as_ref().unwrap();
        let mut w = port.lock().unwrap();
        use std::io::Write;
        w.write_all(data.as_bytes())
            .map_err(|e| AppError::Serial(format!("写入串口失败: {}", e)))?;
        Ok(())
    }

    pub fn disconnect(&mut self) -> Result<(), AppError> {
        self.alive = false;
        if let Some(task) = self.read_task.take() {
            task.abort();
        }
        self.port = None;
        Ok(())
    }
}

#[async_trait::async_trait]
impl ConnHandler for SerialHandler {
    fn id(&self) -> &str {
        &self.id
    }
    fn conn_type(&self) -> ConnType {
        ConnType::Serial
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
        SerialHandler::connect(self, channel)
    }
    async fn write(&self, data: &str) -> Result<(), AppError> {
        SerialHandler::write(self, data)
    }
    async fn disconnect(&mut self) -> Result<(), AppError> {
        SerialHandler::disconnect(self)
    }
    // resize: default no-op (serial has no PTY/window concept).
}
