use anyhow::{Context, Result};
use clap::Parser;

fn main() {
    if let Err(err) = real_main() {
        eprintln!("rustund daemon error: {err:#}");
        std::process::exit(1);
    }
}

#[derive(Debug, Parser)]
#[command(name = "rustund", about = "Run rustun daemon")]
struct Cli {
    #[arg(long)]
    host: String,
    #[arg(long)]
    port: u16,
    #[arg(long)]
    user: String,
    #[arg(long, conflicts_with = "ask_password")]
    password: Option<String>,
    #[arg(long)]
    ask_password: bool,
    #[arg(long)]
    workdir: Option<String>,
}

fn real_main() -> Result<()> {
    let cli = Cli::parse();
    let password = resolve_password(cli.password, cli.ask_password, || {
        rpassword::prompt_password(format!("Password for {}@{}: ", cli.user, cli.host))
            .context("failed to read password from terminal")
    })?;

    let config = rustun::DaemonConfig {
        host: cli.host,
        port: cli.port,
        user: cli.user,
        password,
        workdir: cli.workdir,
    };

    rustun::run_daemon_with_config(config)
}

fn resolve_password<F>(
    provided: Option<String>,
    ask_password: bool,
    mut prompt_fn: F,
) -> Result<Option<String>>
where
    F: FnMut() -> Result<String>,
{
    if let Some(password) = provided {
        return Ok(Some(password));
    }

    if ask_password {
        return Ok(Some(prompt_fn()?));
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_required_remote_options() {
        let cli = Cli::try_parse_from([
            "rustund",
            "--host",
            "127.0.0.1",
            "--port",
            "22",
            "--user",
            "root",
            "--workdir",
            "/srv/app",
        ]);

        let cli = cli.expect("cli should parse");

        assert_eq!(cli.host, "127.0.0.1");
        assert_eq!(cli.port, 22);
        assert_eq!(cli.user, "root");
        assert!(!cli.ask_password);
        assert_eq!(cli.workdir.as_deref(), Some("/srv/app"));
        assert_eq!(cli.password, None);
    }

    #[test]
    fn resolve_password_uses_interactive_when_requested() {
        let password = resolve_password(None, true, || Ok("secret".to_string())).expect("password");
        assert_eq!(password.as_deref(), Some("secret"));
    }

    #[test]
    fn resolve_password_prefers_cli_value() {
        let password = resolve_password(Some("cli-pass".to_string()), true, || {
            panic!("prompt should not run when password already provided")
        })
        .expect("password");

        assert_eq!(password.as_deref(), Some("cli-pass"));
    }

    #[test]
    fn resolve_password_keeps_none_when_not_requested() {
        let password = resolve_password(None, false, || {
            panic!("prompt should not run without --ask-password")
        })
        .expect("password resolution");

        assert_eq!(password, None);
    }
}
