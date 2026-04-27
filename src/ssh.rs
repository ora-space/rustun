use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::Context;
use russh::ChannelMsg;

/// Escape and join program + args for remote shell execution.
pub fn render_remote_command(program: &str, args: &[String]) -> String {
    if args.is_empty() {
        return program.to_string();
    }

    let escaped_args = args
        .iter()
        .map(|arg| shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" ");
    format!("{} {}", program, escaped_args)
}

fn shell_quote(raw: &str) -> String {
    format!("'{}'", raw.replace('\'', "'\\''"))
}

/// Internal SSH client handler used by `russh` callbacks.
pub(crate) struct SshClientHandler;

impl russh::client::Handler for SshClientHandler {
    type Error = russh::Error;

    /// Accept any server key. In the current design we do not perform
    /// host key verification; callers should only use this in trusted
    /// environments or when additional verification is performed elsewhere.
    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        Ok(true)
    }
}

/// Open an authenticated SSH session using `russh` with password auth.
async fn open_ssh_session(
    config: &crate::types::DaemonConfig,
) -> anyhow::Result<russh::client::Handle<SshClientHandler>> {
    let client_config = russh::client::Config {
        inactivity_timeout: Some(Duration::from_secs(30)),
        ..Default::default()
    };
    let client_config = Arc::new(client_config);

    let mut session = russh::client::connect(
        client_config,
        format!("{}:{}", config.host, config.port),
        SshClientHandler,
    )
    .await
    .context("failed to connect ssh server")?;

    let password = config
        .password
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("missing password"))?;

    let auth_result = session
        .authenticate_password(config.user.clone(), password.clone())
        .await
        .context("ssh password authentication failed")?;

    if !auth_result.success() {
        anyhow::bail!("ssh authentication rejected by remote server");
    }

    Ok(session)
}

/// Run the remote command over SSH, bridging messages between the IPC `ipc_reader` and the remote channel.
pub async fn run_remote_command(
    config: crate::types::DaemonConfig,
    remote_command: String,
    mut ipc_reader: crate::socket::LocalStream,
    writer: Arc<Mutex<crate::socket::LocalStream>>,
) -> anyhow::Result<()> {
    let session = open_ssh_session(&config).await?;
    let mut channel = session
        .channel_open_session()
        .await
        .context("failed to open ssh session channel")?;

    channel
        .exec(true, remote_command)
        .await
        .context("failed to execute remote command")?;

    let (stdin_tx, mut stdin_rx) = tokio::sync::mpsc::unbounded_channel::<crate::types::ClientMessage>();
    let stdin_thread = thread::spawn(move || -> std::io::Result<()> {
        while let Some(msg) = crate::codec::recv_message::<_, crate::types::ClientMessage>(&mut ipc_reader)? {
            if stdin_tx.send(msg).is_err() {
                break;
            }
        }
        Ok(())
    });

    let mut exit_code: i32 = 1;
    let mut stdin_closed = false;

    loop {
        tokio::select! {
            maybe_msg = stdin_rx.recv(), if !stdin_closed => {
                match maybe_msg {
                    Some(crate::types::ClientMessage::Stdin { data }) => {
                        channel
                            .data(&data[..])
                            .await
                            .context("failed to send stdin to remote")?;
                    }
                    Some(crate::types::ClientMessage::CloseStdin) => {
                        channel.eof().await.context("failed to send remote eof")?;
                        stdin_closed = true;
                    }
                    Some(crate::types::ClientMessage::Exec { .. }) => {
                        anyhow::bail!("unexpected Exec message after startup");
                    }
                    None => {
                        stdin_closed = true;
                    }
                }
            }
            channel_msg = channel.wait() => {
                let Some(channel_msg) = channel_msg else {
                    break;
                };

                match channel_msg {
                    ChannelMsg::Data { data } => {
                        crate::socket::send_server(
                            &writer,
                            crate::types::ServerMessage::Stdout { data: data.to_vec() },
                        )?;
                    }
                    ChannelMsg::ExtendedData { data, .. } => {
                        crate::socket::send_server(
                            &writer,
                            crate::types::ServerMessage::Stderr { data: data.to_vec() },
                        )?;
                    }
                    ChannelMsg::ExitStatus { exit_status } => {
                        exit_code = exit_status as i32;
                    }
                    ChannelMsg::Close => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    if !stdin_closed {
        let _ = channel.eof().await;
    }
    let _ = channel.close().await;
    let _ = session
        .disconnect(russh::Disconnect::ByApplication, "", "English")
        .await;

    let _ = stdin_thread;
    crate::socket::send_server(&writer, crate::types::ServerMessage::Exit { code: exit_code })?;

    Ok(())
}
