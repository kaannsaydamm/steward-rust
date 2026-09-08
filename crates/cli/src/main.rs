mod artifact_commands;
mod channel_commands;
mod cli;
mod client;
mod client_chat;
mod commands;
mod cron_commands;
mod daemon_lifecycle;
mod dashboard_commands;
mod data_archive;
mod doctor;
mod interactive;
mod interactive_commands;
mod interactive_help;
mod interactive_registry;
mod logs_commands;
mod mcp_commands;
mod operator_status;
mod prompts;
mod provider_commands;
mod registry_commands;
mod registry_view;
mod security_commands;
mod session_commands;
mod setup;
mod tui;
mod ui;
mod workflow_view;
mod workflow_watch;

use anyhow::Result;
use clap::{CommandFactory as _, Parser};
use cli::Cli;
use cli::Command;
use steward_core::pb::ChatEventKind;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .without_time()
        .with_writer(std::io::stderr)
        .compact()
        .init();

    let cli = Cli::parse();
    if let Some(Command::Completion(args)) = cli.command.as_ref() {
        let mut command = Cli::command();
        let name = command.get_name().to_owned();
        clap_complete::generate(args.shell, &mut command, name, &mut std::io::stdout());
        return Ok(());
    }
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
    let is_dashboard = matches!(cli.command.as_ref(), Some(Command::Dashboard(_)));
    let is_logs = matches!(cli.command.as_ref(), Some(Command::Logs(_)));
    if !is_doctor && !is_data && !is_dashboard && !is_logs {
        daemon_lifecycle::ensure_running(&host, auto_start, settings.web_port).await?;
    }
    if cli.command.is_none() && cli.oneshot.is_none() && auto_start {
        daemon_lifecycle::ensure_web_ready(settings.web_port).await?;
    }
    if let Some(prompt) = cli.oneshot {
        return run_oneshot(&host, &prompt).await;
    }
    match cli.command {
        Some(command) => {
            commands::run(
                &host,
                auto_start,
                &settings.web_url(),
                settings.web_port,
                command,
            )
            .await
        }
        None => {
            // The ratatui TUI is retired as the default surface: the product
            // CLI/TUI is the OMP-derived `steward` (npm launcher). Management
            // subcommands above stay; bare `steward-cli` points there.
            println!("The interactive Steward TUI is the `steward` command (npm i -g steward).");
            println!("`steward-cli` keeps daemon management subcommands: try `steward-cli --help`.");
            Ok(())
        }
    }
}

/// One-shot mode: send a single prompt and print only the final response text, matching
/// scriptable one-shot flags in comparable CLIs (e.g. `-z`/`--oneshot`). No banner, no
/// spinner, no tool-call previews; governed tools still run with approval auto-bypassed.
async fn run_oneshot(host: &str, prompt: &str) -> Result<()> {
    let events = client_chat::chat(host, "", prompt, true).await?;
    for event in events {
        if event.kind == ChatEventKind::Text as i32 {
            print!("{}", event.content);
        }
    }
    println!();
    Ok(())
}
