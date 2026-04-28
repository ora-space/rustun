use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::Context;

/// Start the daemon using environment-derived defaults.
pub fn run_daemon() -> anyhow::Result<()> {
    let default_user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string());

    run_daemon_with_config(crate::types::DaemonConfig {
        host: "127.0.0.1".to_string(),
        port: 22,
        user: default_user,
        password: std::env::var("RUSTUN_PASSWORD").ok(),
        workdir: None,
    })
}

/// Start the daemon using an explicit config.
pub fn run_daemon_with_config(config: crate::types::DaemonConfig) -> anyhow::Result<()> {
    validate_daemon_config(&config)?;

    let auth_mode = if config.password.is_some() {
        "password"
    } else {
        "default"
    };
    eprintln!(
        "rustund target: {}@{}:{} (auth: {})",
        config.user, config.host, config.port, auth_mode
    );

    let socket_path = crate::socket::socket_path()?;
    crate::socket::remove_socket_file_if_exists(&socket_path)?;

    let listener = crate::socket::bind_socket(&socket_path)
        .with_context(|| format!("failed to bind socket at {}", socket_path.display()))?;
    let _guard = crate::socket::SocketFileGuard::new(socket_path.clone());

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_for_handler = Arc::clone(&shutdown);
    let wake_path = socket_path.clone();

    ctrlc::set_handler(move || {
        shutdown_for_handler.store(true, Ordering::SeqCst);
        let _ = crate::socket::connect_socket(&wake_path);
    })
    .context("failed to install ctrl-c handler")?;

    while !shutdown.load(Ordering::SeqCst) {
        match crate::socket::accept_connection(&listener) {
            Ok(stream) => {
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }

                let client_config = config.clone();
                thread::spawn(move || {
                    if let Err(err) = handle_client(stream, client_config) {
                        eprintln!("client error: {:#}", err);
                    }
                });
            }
            Err(err) if shutdown.load(Ordering::SeqCst) => {
                if err.kind() != io::ErrorKind::Interrupted {
                    break;
                }
            }
            Err(err) => {
                eprintln!("accept error: {}", err);
            }
        }
    }

    Ok(())
}

/// Validate that required fields are present for SSH authentication.
pub(crate) fn validate_daemon_config(config: &crate::types::DaemonConfig) -> anyhow::Result<()> {
    if config.host.trim().is_empty() {
        anyhow::bail!("host must not be empty");
    }
    if config.user.trim().is_empty() {
        anyhow::bail!("user must not be empty");
    }
    if config
        .password
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .is_empty()
    {
        anyhow::bail!("password is required for russh password authentication");
    }
    Ok(())
}

/// Handle a single accepted client connection: read Exec, then run SSH command bridge.
fn handle_client(
    stream: crate::socket::LocalStream,
    config: crate::types::DaemonConfig,
) -> anyhow::Result<()> {
    let mut reader = stream
        .try_clone()
        .context("failed to clone accepted stream for reading")?;
    let writer = Arc::new(Mutex::new(stream));

    let first = crate::codec::recv_message::<_, crate::types::ClientMessage>(&mut reader)?
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "missing first message")
        })?;

    let (program, args) = match first {
        crate::types::ClientMessage::Exec { program, args } => (program, args),
        _ => {
            crate::socket::send_server(
                &writer,
                crate::types::ServerMessage::Error {
                    message: "first message must be Exec".to_string(),
                },
            )?;
            crate::socket::send_server(&writer, crate::types::ServerMessage::Exit { code: 2 })?;
            return Ok(());
        }
    };

    let remote_command =
        crate::ssh::render_remote_command_with_workdir(config.workdir.as_deref(), &program, &args);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime")?;

    let result = runtime.block_on(crate::ssh::run_remote_command(
        config,
        remote_command,
        reader,
        Arc::clone(&writer),
    ));

    if let Err(err) = result {
        crate::socket::send_server(
            &writer,
            crate::types::ServerMessage::Error {
                message: format!("ssh execution failed: {}", err),
            },
        )?;
        crate::socket::send_server(&writer, crate::types::ServerMessage::Exit { code: 255 })?;
    }

    Ok(())
}
