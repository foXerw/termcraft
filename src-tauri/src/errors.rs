use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("SSH error: {0}")]
    Ssh(String),

    #[error("Telnet error: {0}")]
    Telnet(String),

    #[error("Local shell error: {0}")]
    LocalShell(String),

    #[error("Preset error: {0}")]
    Preset(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
}

impl From<AppError> for String {
    fn from(err: AppError) -> String {
        err.to_string()
    }
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}