use crate::cli::McpCommand;
use crate::client;
use crate::ui::HistoryLine;
use anyhow::Result;

pub async fn run(host: &str, command: McpCommand) -> Result<()> {
    match command {
        McpCommand::Add(args) => {
            let cwd = args
                .cwd
                .map(|path| path.display().to_string())
                .unwrap_or_default();
            let adapter = client::register_mcp(
                host,
                &args.adapter_id,
                &args.name,
                &args.command,
                args.arguments,
                &cwd,
            )
            .await?;
            println!("registered\t{}", adapter.adapter_id);
        }
        McpCommand::List => print_adapters(client::list_mcp(host).await?),
        McpCommand::Start(args) => {
            println!(
                "{}",
                adapter_line(&client::start_mcp(host, &args.adapter_id).await?)
            );
        }
        McpCommand::Stop(args) => {
            println!(
                "{}",
                adapter_line(&client::stop_mcp(host, &args.adapter_id).await?)
            );
        }
        McpCommand::Remove(args) => {
            let removed = client::remove_mcp(host, &args.adapter_id).await?;
            println!("removed={removed}\t{}", args.adapter_id);
        }
    }
    Ok(())
}

pub async fn list_lines(host: &str) -> Result<Vec<HistoryLine>> {
    Ok(client::list_mcp(host)
        .await?
        .iter()
        .map(adapter_line)
        .map(HistoryLine::agent)
        .collect())
}

pub async fn action_line(host: &str, adapter_id: &str, start: bool) -> Result<HistoryLine> {
    let adapter = if start {
        client::start_mcp(host, adapter_id).await?
    } else {
        client::stop_mcp(host, adapter_id).await?
    };
    Ok(HistoryLine::system(adapter_line(&adapter)))
}

fn print_adapters(adapters: Vec<steward_core::pb::McpAdapterInfo>) {
    for adapter in adapters {
        println!("{}", adapter_line(&adapter));
    }
}

fn adapter_line(adapter: &steward_core::pb::McpAdapterInfo) -> String {
    let error = if adapter.error.is_empty() {
        String::new()
    } else {
        format!("\terror={}", adapter.error.replace('\n', " "))
    };
    format!(
        "{}\tstatus={}\ttools={}\tserver={}@{}\t{}{}",
        adapter.adapter_id,
        adapter.status,
        adapter.tool_count,
        adapter.server_name,
        adapter.server_version,
        adapter.name,
        error
    )
}
