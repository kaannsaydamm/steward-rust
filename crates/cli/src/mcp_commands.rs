use crate::cli::McpCommand;
use crate::client;
use crate::ui::HistoryLine;
use anyhow::{bail, Result};

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
        McpCommand::Catalog => print_catalog(client::list_mcp_catalog(host).await?),
        McpCommand::SearchMarketplace(args) => {
            print_connector_search(client::search_connector_marketplace(host, &args.query).await?)
        }
        McpCommand::QuickAdd(args) => {
            let adapter =
                quick_add(host, &args.catalog_id, &args.adapter_id, args.extra_args).await?;
            println!("registered\t{}", adapter.adapter_id);
        }
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

pub async fn catalog_lines(host: &str) -> Result<Vec<HistoryLine>> {
    Ok(client::list_mcp_catalog(host)
        .await?
        .iter()
        .map(catalog_line)
        .map(HistoryLine::agent)
        .collect())
}

pub async fn search_marketplace_lines(host: &str, query: &str) -> Result<Vec<HistoryLine>> {
    let (entries, error) = client::search_connector_marketplace(host, query).await?;
    if !error.is_empty() {
        return Ok(vec![HistoryLine::error(error)]);
    }
    Ok(entries
        .iter()
        .map(connector_line)
        .map(HistoryLine::agent)
        .collect())
}

pub async fn quick_add_lines(
    host: &str,
    catalog_id: &str,
    adapter_id: &str,
    extra_args: Vec<String>,
) -> Result<HistoryLine> {
    let adapter = quick_add(host, catalog_id, adapter_id, extra_args).await?;
    Ok(HistoryLine::system(format!(
        "registered\t{}",
        adapter.adapter_id
    )))
}

async fn quick_add(
    host: &str,
    catalog_id: &str,
    adapter_id: &str,
    extra_args: Vec<String>,
) -> Result<steward_core::pb::McpAdapterInfo> {
    let catalog = client::list_mcp_catalog(host).await?;
    let Some(entry) = catalog
        .into_iter()
        .find(|entry| entry.catalog_id == catalog_id)
    else {
        bail!("unknown catalog entry '{catalog_id}' — see `steward mcp catalog`");
    };
    let mut arguments = entry.args;
    arguments.extend(extra_args);
    client::register_mcp(host, adapter_id, &entry.name, &entry.command, arguments, "").await
}

fn print_adapters(adapters: Vec<steward_core::pb::McpAdapterInfo>) {
    for adapter in adapters {
        println!("{}", adapter_line(&adapter));
    }
}

fn print_catalog(entries: Vec<steward_core::pb::McpCatalogEntry>) {
    for entry in entries {
        println!("{}", catalog_line(&entry));
    }
}

fn print_connector_search(result: (Vec<steward_core::pb::ConnectorMarketplaceEntry>, String)) {
    let (entries, error) = result;
    if !error.is_empty() {
        eprintln!("error: {error}");
        return;
    }
    for entry in entries {
        println!("{}", connector_line(&entry));
    }
}

fn connector_line(entry: &steward_core::pb::ConnectorMarketplaceEntry) -> String {
    let kind = if entry.remote { "remote" } else { "local" };
    format!(
        "{}\t{}\tverified={}\tuses={}\t{}",
        entry.qualified_name,
        kind,
        entry.verified,
        entry.use_count,
        entry.description.replace('\n', " ")
    )
}

fn catalog_line(entry: &steward_core::pb::McpCatalogEntry) -> String {
    format!(
        "{}\t{} {}\tpublisher={}\t{}",
        entry.catalog_id,
        entry.command,
        entry.args.join(" "),
        entry.publisher,
        entry.description.replace('\n', " ")
    )
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
