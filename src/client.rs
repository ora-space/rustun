use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::Context;

/// Run the local client helper. Connects to the daemon socket, sends the Exec
/// message, forwards stdin, and prints back server stdout/stderr until Exit.
pub fn run_client(program: String, args: Vec<String>) -> anyhow::Result<i32> {
    let socket = crate::socket::socket_path()?;
    let mut writer = crate::socket::connect_socket(&socket)
        .with_context(|| format!("failed to connect daemon socket at {}", socket.display()))?;
    let mut reader = writer
        .try_clone()
        .context("failed to clone socket stream for reading")?;

    crate::codec::send_message(&mut writer, &crate::types::ClientMessage::Exec { program, args })?;

    let writer = Arc::new(Mutex::new(writer));
    let writer_for_stdin = Arc::clone(&writer);
    let stdin_thread = thread::spawn(move || forward_stdin(writer_for_stdin));

    let mut stdout = io::stdout();
    let mut stderr = io::stderr();
    let mut exit_code = 1;

    while let Some(msg) = crate::codec::recv_message::<_, crate::types::ServerMessage>(&mut reader)? {
        match msg {
            crate::types::ServerMessage::Stdout { data } => {
                stdout.write_all(&data)?;
                stdout.flush()?;
            }
            crate::types::ServerMessage::Stderr { data } => {
                stderr.write_all(&data)?;
                stderr.flush()?;
            }
            crate::types::ServerMessage::Error { message } => {
                writeln!(stderr, "rustund error: {}", message)?;
                stderr.flush()?;
            }
            crate::types::ServerMessage::Exit { code } => {
                exit_code = code;
                break;
            }
        }
    }

    let _ = stdin_thread;

    Ok(exit_code)
}

/// Read from local stdin and forward messages to the daemon via `writer`.
fn forward_stdin(writer: Arc<Mutex<crate::socket::LocalStream>>) -> anyhow::Result<()> {
    let mut stdin = io::stdin();
    let mut buffer = [0_u8; 8192];

    loop {
        let read = stdin.read(&mut buffer)?;
        if read == 0 {
            let mut guard = writer
                .lock()
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "stdin writer mutex poisoned"))?;
            crate::codec::send_message(&mut *guard, &crate::types::ClientMessage::CloseStdin)?;
            return Ok(());
        }

        let mut guard = writer
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "stdin writer mutex poisoned"))?;
        crate::codec::send_message(
            &mut *guard,
            &crate::types::ClientMessage::Stdin {
                data: buffer[..read].to_vec(),
            },
        )?;
    }
}
