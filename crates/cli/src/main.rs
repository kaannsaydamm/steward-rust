mod cli;
mod client;
mod commands;
mod daemon_lifecycle;
mod interactive;
mod interactive_commands;
mod interactive_help;
mod operator_status;
mod tui;
mod ui;
mod workflow_view;
mod workflow_watch;

use anyhow::Result;
use clap::Parser;
use cli::Cli;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .without_time()
        .with_writer(std::io::stderr)
        .compact()
        .init();

    let cli = Cli::parse();
    daemon_lifecycle::ensure_running(&cli.host, !cli.no_auto_start).await?;
    match cli.command {
        Some(command) => commands::run(&cli.host, command).await,
        None => interactive::run(cli.host).await,
    }
}
