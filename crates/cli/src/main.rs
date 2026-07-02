mod cli;
mod client;
mod client_chat;
mod commands;
mod daemon_lifecycle;
mod data_archive;
mod doctor;
mod interactive;
mod interactive_commands;
mod interactive_help;
mod interactive_registry;
mod mcp_commands;
mod operator_status;
mod provider_commands;
mod registry_commands;
mod registry_view;
mod session_commands;
mod setup;
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
    let settings_path = setup::settings_path()?;
    if let Some(Command::Setup(args)) = cli.command.as_ref() {
        let settings = setup::run(&settings_path, args)?;
        let default_host = settings.daemon_url();
        let host = cli.host.as_deref().unwrap_or(&default_host).to_owned();
        daemon_lifecycle::ensure_running(&host, auto_start, settings.web_port).await?;
        if auto_start {
            daemon_lifecycle::ensure_web_ready(settings.web_port).await?;
        }
        println!("Daemon: {host}");
        println!("Web UI: {}", settings.web_url());
        return Ok(());
    }
    let settings = if cli.command.is_none() && setup::needs_setup(&settings_path) {
        setup::run(
            &settings_path,
            &cli::SetupArgs {
                quick: false,
                web_port: None,
                provider: None,
                model: None,
                base_url: None,
                api_key_env: None,
                skip_provider: false,
            },
        )?
    } else {
        setup::load_or_default(&settings_path)?
    };
    let host = cli.host.unwrap_or_else(|| settings.daemon_url());
    let is_doctor = matches!(cli.command.as_ref(), Some(Command::Doctor(_)));
    let is_data = matches!(cli.command.as_ref(), Some(Command::Data(_)));
    if !is_doctor && !is_data {
        daemon_lifecycle::ensure_running(&host, auto_start, settings.web_port).await?;
    }
    if cli.command.is_none() && auto_start {
        daemon_lifecycle::ensure_web_ready(settings.web_port).await?;
    }
    match cli.command {
        Some(command) => commands::run(&host, auto_start, command).await,
        None => interactive::run(host, settings.web_url()).await,
    }
}
