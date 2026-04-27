use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::Context;

/// Socket file name placed in the user's home directory.
const SOCKET_FILE_NAME: &str = "rustun.sock";

#[cfg(unix)]
pub type LocalListener = std::os::unix::net::UnixListener;
#[cfg(unix)]
pub type LocalStream = std::os::unix::net::UnixStream;

#[cfg(windows)]
pub type LocalListener = uds_windows::UnixListener;
#[cfg(windows)]
pub type LocalStream = uds_windows::UnixStream;

/// Return the path to the per-user socket file.
pub fn socket_path() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().context("failed to determine home directory")?;
    Ok(home.join(SOCKET_FILE_NAME))
}

/// Bind a listener to `path`.
pub fn bind_socket(path: &Path) -> io::Result<LocalListener> {
    LocalListener::bind(path)
}

/// Connect to a unix domain socket at `path`.
pub fn connect_socket(path: &Path) -> io::Result<LocalStream> {
    LocalStream::connect(path)
}

/// Accept a single connection from `listener` and return the connected stream.
pub fn accept_connection(listener: &LocalListener) -> io::Result<LocalStream> {
    listener.accept().map(|(stream, _addr)| stream)
}

/// Remove the socket file if it already exists.
pub fn remove_socket_file_if_exists(path: &Path) -> io::Result<()> {
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// Guard object that ensures the socket file is removed when dropped.
pub struct SocketFileGuard {
    path: PathBuf,
}

impl SocketFileGuard {
    /// Create a new guard for `path`.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for SocketFileGuard {
    fn drop(&mut self) {
        let _ = remove_socket_file_if_exists(&self.path);
    }
}

/// Helper to send a `ServerMessage` through a locked `LocalStream`.
pub fn send_server(writer: &Arc<Mutex<LocalStream>>, msg: crate::types::ServerMessage) -> anyhow::Result<()> {
    let mut guard = writer
        .lock()
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "socket writer mutex poisoned"))?;
    crate::codec::send_message(&mut *guard, &msg)?;
    Ok(())
}
