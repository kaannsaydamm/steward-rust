mod cli;
mod client;
mod commands;
mod daemon_lifecycle;
mod doctor;
mod interactive;
mod interactive_commands;
mod interactive_help;
mod interactive_registry;
mod operator_status;
mod registry_commands;
mod registry_view;
mod tui;
mod ui;
mod workflow_view;
mod workflow_watch;

use anyhow::Result;
use clap::Parser;
use cli::Cli;
use cli::Command;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .without_time()
        .with_writer(std::io::stderr)
        .compact()
        .init();

    let cli = Cli::parse();
    let auto_start = !cli.no_auto_start;
    let is_doctor = matches!(cli.command.as_ref(), Some(Command::Doctor(_)));
    if !is_doctor {
        daemon_lifecycle::ensure_running(&cli.host, auto_start).await?;
    }
    match cli.command {
        Some(command) => commands::run(&cli.host, auto_start, command).await,
        None => interactive::run(cli.host).await,
    }
}
