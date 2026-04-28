use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "rustun", about = "Run command through rustund daemon")]
struct Cli {
    #[arg(required = true, value_name = "COMMAND")]
    command: String,
}

fn main() {
    let cli = Cli::parse();
    let code = match rustun::run_client(cli.command) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("rustun client error: {err:#}");
            1
        }
    };

    std::process::exit(code);
}
