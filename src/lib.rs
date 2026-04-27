//! Library entrypoint for the `rustun` crate.
//!
//! This crate is split into focused modules to keep responsibilities clear:
//! - `types` contains shared data types used across the crate
//! - `socket` contains platform socket helpers and the socket guard
//! - `codec` contains the bincode length-prefixed send/recv helpers
//! - `client` contains the local client helper used by the `rustun` binary
//! - `daemon` contains the daemon main loop and connection handling
//! - `ssh` contains SSH-specific logic and remote execution bridging

pub mod types;
pub mod socket;
pub mod codec;
pub mod client;
pub mod daemon;
pub mod ssh;

// Re-export the small stable public surface used by the binaries so they
// can continue calling `rustun::run_client` and `rustun::run_daemon_with_config`.
pub use client::run_client;
pub use daemon::{run_daemon, run_daemon_with_config};
pub use types::DaemonConfig;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn client_message_round_trip() {
        let msg = types::ClientMessage::Exec {
            program: "echo".to_string(),
            args: vec!["hello".to_string()],
        };

        let mut bytes = Vec::new();
        codec::send_message(&mut bytes, &msg).expect("serialize client message");

        let mut cursor = Cursor::new(bytes);
        let decoded = codec::recv_message::<_, types::ClientMessage>(&mut cursor)
            .expect("deserialize client message")
            .expect("client message present");

        assert_eq!(decoded, msg);
    }

    #[test]
    fn server_message_round_trip() {
        let msg = types::ServerMessage::Stderr {
            data: b"oops".to_vec(),
        };

        let mut bytes = Vec::new();
        codec::send_message(&mut bytes, &msg).expect("serialize server message");

        let mut cursor = Cursor::new(bytes);
        let decoded = codec::recv_message::<_, types::ServerMessage>(&mut cursor)
            .expect("deserialize server message")
            .expect("server message present");

        assert_eq!(decoded, msg);
    }

    #[test]
    fn socket_path_uses_home_directory_and_file_name() {
        let path = socket::socket_path().expect("socket path");
        let home = dirs::home_dir().expect("home directory available");

        assert!(path.starts_with(home));
        assert_eq!(
            path.file_name().and_then(|s| s.to_str()),
            Some("rustun.sock")
        );
    }

    #[test]
    fn render_remote_command_quotes_arguments() {
        let rendered = ssh::render_remote_command("printf", &["hello world".to_string(), "x'y".to_string()]);

        assert_eq!(rendered, "printf 'hello world' 'x'\\''y'");
    }

    #[test]
    fn daemon_config_requires_password_for_ssh_mode() {
        let config = types::DaemonConfig {
            host: "127.0.0.1".to_string(),
            port: 22,
            user: "root".to_string(),
            password: None,
        };

        let err = daemon::validate_daemon_config(&config).expect_err("password should be required");
        assert!(err.to_string().contains("password"));
    }
}
