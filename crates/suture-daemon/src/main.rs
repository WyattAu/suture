use clap::Parser;
use suture_daemon::{DaemonCommand, execute_command};

#[derive(Parser)]
#[command(
    name = "suture-daemon",
    about = "Suture background daemon for file watching and automatic sync"
)]
struct Cli {
    #[command(subcommand)]
    command: DaemonCommand,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("suture_daemon=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();

    if let Err(e) = execute_command(cli.command).await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
