use serde::{Deserialize, Serialize};

/// Configuration used to connect to the remote SSH target.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: Option<String>,
    pub workdir: Option<String>,
}

/// Messages sent from the client-side helper to the daemon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClientMessage {
    Exec { program: String, args: Vec<String> },
    Stdin { data: Vec<u8> },
    CloseStdin,
}

/// Messages sent from the daemon back to the client-side helper.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ServerMessage {
    Stdout { data: Vec<u8> },
    Stderr { data: Vec<u8> },
    Exit { code: i32 },
    Error { message: String },
}
