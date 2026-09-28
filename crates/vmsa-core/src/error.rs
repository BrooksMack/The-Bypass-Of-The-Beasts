use thiserror::Error;

/// Errors produced by vmsa-core. Every variant carries a message that is safe to show
/// to a nontechnical user; technical detail goes into `detail` fields or logs.
#[derive(Debug, Error, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CoreError {
    #[error("VirtualBox is not installed or VBoxManage could not be found")]
    VirtualBoxNotFound,

    #[error("VBoxManage command failed (exit code {code:?}): {stderr}")]
    VBoxManageFailed {
        args: Vec<String>,
        code: Option<i32>,
        stdout: String,
        stderr: String,
    },

    #[error("VBoxManage did not respond within {seconds} seconds")]
    VBoxManageTimeout { seconds: u64 },

    #[error("Could not parse VBoxManage output: {0}")]
    Parse(String),

    #[error("This computer cannot run the requested setup: {0}")]
    Unsupported(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Not enough free disk space at {path}: {available_bytes} bytes available, {required_bytes} bytes needed")]
    InsufficientDisk {
        path: String,
        available_bytes: u64,
        required_bytes: u64,
    },

    #[error("Download failed: {0}")]
    Download(String),

    #[error("Download verification failed for {file}: expected SHA-256 {expected}, got {actual}")]
    ChecksumMismatch {
        file: String,
        expected: String,
        actual: String,
    },

    #[error("The operation was cancelled")]
    Cancelled,

    #[error("Another setup operation is already running")]
    Busy,

    #[error("A virtual machine named \"{name}\" already exists and was not created by this app")]
    VmNameConflict { name: String, uuid: String },

    #[error("File error at {path}: {message}")]
    Io { path: String, message: String },

    #[error("{0}")]
    Other(String),
}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        CoreError::Io {
            path: String::new(),
            message: e.to_string(),
        }
    }
}

impl From<serde_json::Error> for CoreError {
    fn from(e: serde_json::Error) -> Self {
        CoreError::Parse(e.to_string())
    }
}

impl From<anyhow::Error> for CoreError {
    fn from(e: anyhow::Error) -> Self {
        CoreError::Other(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;

impl CoreError {
    pub fn io(path: impl AsRef<std::path::Path>, e: std::io::Error) -> Self {
        CoreError::Io {
            path: path.as_ref().display().to_string(),
            message: e.to_string(),
        }
    }
}
