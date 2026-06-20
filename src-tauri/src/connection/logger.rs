use std::fs::OpenOptions;
use std::io::Write;

use strip_ansi_escapes::Writer as StripWriter;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::errors::AppError;

/// A chunk of session data to be appended to the log file.
#[derive(Debug)]
pub enum LogChunk {
    Output(Vec<u8>),
    Input(Vec<u8>),
}

/// Handle for an active per-connection log. Dropping the `sender` (e.g. by
/// removing the handle from the manager) ends the writer task and closes the
/// file. `out_sub_id` is used to detach the output-tap subscriber on stop.
pub struct LoggerHandle {
    pub sender: UnboundedSender<LogChunk>,
    pub out_sub_id: u64,
}

impl LoggerHandle {
    /// Open `path` for append and spawn a writer task that consumes both the
    /// connection's output stream (`rx_out`) and input chunks (`log_rx`,
    /// fed by `log_input`), writing them to the file in arrival order. ANSI
    /// escape sequences (CSI/OSC/etc.) are stripped via a stateful streaming
    /// writer so the log is readable plain text; incomplete sequences split
    /// across chunks are handled correctly.
    pub fn start(
        path: &str,
        mut rx_out: UnboundedReceiver<Vec<u8>>,
        out_sub_id: u64,
    ) -> Result<Self, AppError> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .truncate(false)
            .open(path)
            .map_err(|e| AppError::Connection(format!("打开日志文件失败: {}", e)))?;
        let mut writer = StripWriter::new(file);

        let (log_tx, mut log_rx) = unbounded_channel::<LogChunk>();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    out = rx_out.recv() => match out {
                        Some(bytes) => {
                            let _ = writer.write_all(&bytes);
                            let _ = writer.flush();
                        }
                        None => break,
                    },
                    inp = log_rx.recv() => match inp {
                        Some(LogChunk::Output(bytes)) | Some(LogChunk::Input(bytes)) => {
                            let _ = writer.write_all(&bytes);
                            let _ = writer.flush();
                        }
                        None => break,
                    },
                }
            }
        });

        Ok(LoggerHandle {
            sender: log_tx,
            out_sub_id,
        })
    }
}
