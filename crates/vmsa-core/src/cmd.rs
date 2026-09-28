//! Typed, structured process execution. Arguments are always passed as an array;
//! nothing is ever interpolated into a shell string.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio_util_lite::CancellationToken;

use crate::{CoreError, Result};

/// Minimal cancellation token so `vmsa-core` does not depend on tokio-util.
pub mod tokio_util_lite {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use tokio::sync::Notify;

    #[derive(Clone, Default, Debug)]
    pub struct CancellationToken {
        inner: Arc<Inner>,
    }

    #[derive(Default, Debug)]
    struct Inner {
        cancelled: AtomicBool,
        notify: Notify,
    }

    impl CancellationToken {
        pub fn new() -> Self {
            Self::default()
        }
        pub fn cancel(&self) {
            self.inner.cancelled.store(true, Ordering::SeqCst);
            self.inner.notify.notify_waiters();
        }
        pub fn is_cancelled(&self) -> bool {
            self.inner.cancelled.load(Ordering::SeqCst)
        }
        pub async fn cancelled(&self) {
            loop {
                if self.is_cancelled() {
                    return;
                }
                self.inner.notify.notified().await;
            }
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommandOutput {
    pub program: String,
    pub args: Vec<String>,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u128,
}

impl CommandOutput {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// Run a program with an argument array, a timeout and optional cancellation.
/// Output is captured as UTF-8 (lossy) so localized or odd bytes never panic.
pub async fn run(
    program: &Path,
    args: &[String],
    timeout: Duration,
    cancel: Option<&CancellationToken>,
    cwd: Option<&Path>,
) -> Result<CommandOutput> {
    let started = std::time::Instant::now();
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    // Ask VirtualBox for untranslated output where it honours the locale.
    cmd.env("LANG", "C").env("LC_ALL", "C");
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(|e| CoreError::Io {
        path: program.display().to_string(),
        message: format!("could not start process: {e}"),
    })?;

    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let read_out = async {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf).await;
        buf
    };
    let read_err = async {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf).await;
        buf
    };

    let wait = async {
        let (out, err) = tokio::join!(read_out, read_err);
        let status = child.wait().await;
        (out, err, status)
    };

    let cancel_fut = async {
        match cancel {
            Some(c) => c.cancelled().await,
            None => std::future::pending::<()>().await,
        }
    };

    tokio::select! {
        res = tokio::time::timeout(timeout, wait) => {
            match res {
                Ok((out, err, status)) => {
                    let status = status.map_err(|e| CoreError::Io { path: program.display().to_string(), message: e.to_string() })?;
                    Ok(CommandOutput {
                        program: program.display().to_string(),
                        args: args.to_vec(),
                        code: status.code(),
                        stdout: String::from_utf8_lossy(&out).into_owned(),
                        stderr: String::from_utf8_lossy(&err).into_owned(),
                        duration_ms: started.elapsed().as_millis(),
                    })
                }
                Err(_) => Err(CoreError::VBoxManageTimeout { seconds: timeout.as_secs() }),
            }
        }
        _ = cancel_fut => {
            Err(CoreError::Cancelled)
        }
    }
}

/// Locate a program on PATH or at candidate absolute paths.
pub fn find_program(name: &str, candidates: &[PathBuf]) -> Option<PathBuf> {
    for c in candidates {
        if c.is_file() {
            return Some(c.clone());
        }
    }
    which::which(name).ok()
}
