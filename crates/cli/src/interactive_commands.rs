use crate::client;
use crate::doctor;
use crate::interactive_registry;
use crate::mcp_commands;
use crate::operator_status;
use crate::registry_view;
use crate::ui::HistoryLine;
use crate::workflow_view;
use crate::workflow_watch::{self, WatchOptions};
use anyhow::Result;

pub struct DispatchResult {
    pub lines: Vec<HistoryLine>,
    pub daemon_status: Option<String>,
}

impl DispatchResult {
    fn lines(lines: Vec<HistoryLine>) -> Self {
        Self {
            lines,
            daemon_status: None,
        }
    }

    fn with_daemon_status(lines: Vec<HistoryLine>, status: String) -> Self {
        Self {
            lines,
            daemon_status: Some(status),
        }
    }
}

pub async fn dispatch(host: &str, command: &str) -> Result<DispatchResult> {
    if command == "/ping" {
        let status = client::ping(host).await?;
        return Ok(DispatchResult::with_daemon_status(
            vec![HistoryLine::system(format!("daemon: {status}"))],
            format!("online: {status}"),
        ));
    }
    if command == "/status" {
        let status = operator_status::load(host).await?;
        let lines = status
            .lines()
            .into_iter()
            .map(HistoryLine::system)
            .collect();
        return Ok(DispatchResult::with_daemon_status(
            lines,
            format!("online: {}", status.daemon),
        ));
    }
    if command == "/doctor" {
        let report = doctor::collect(host, false).await;
        let lines = report
            .lines()
            .into_iter()
            .map(HistoryLine::system)
            .collect();
        return Ok(DispatchResult::lines(lines));
    }
    if command == "/agents" {
        let agents = client::list_agents(host).await?;
        let mut lines = vec![HistoryLine::system(format!("{} agents", agents.len()))];
        for agent in agents {
            lines.push(HistoryLine::agent(format!(
                "{} | {} | {} | {:.0}%",
                agent.agent_id,
                agent.name,
                agent.status,
                agent.progress * 100.0
            )));
        }
        return Ok(DispatchResult::lines(lines));
    }
    if command == "/tools" {
        return Ok(DispatchResult::lines(
            interactive_registry::tool_lines(host).await?,
        ));
    }
    if command == "/skills" {
        return Ok(DispatchResult::lines(
            interactive_registry::skill_lines(host).await?,
        ));
    }
    if command == "/mcp" {
        return Ok(DispatchResult::lines(mcp_commands::list_lines(host).await?));
    }
    if let Some(id) = command.strip_prefix("/mcp-start ") {
        return Ok(DispatchResult::lines(vec![
            mcp_commands::action_line(host, id.trim(), true).await?,
        ]));
    }
    if let Some(id) = command.strip_prefix("/mcp-stop ") {
        return Ok(DispatchResult::lines(vec![
            mcp_commands::action_line(host, id.trim(), false).await?,
        ]));
    }
    if let Some(raw) = command.strip_prefix("/mcp-add ") {
        return mcp_add_lines(host, raw).await;
    }
    if let Some(id) = command.strip_prefix("/mcp-remove ") {
        let removed = client::remove_mcp(host, id.trim()).await?;
        return Ok(DispatchResult::lines(vec![HistoryLine::system(format!(
            "removed={removed}\t{}",
            id.trim()
        ))]));
    }
    if let Some(id) = command.strip_prefix("/enable ") {
        let tool = client::set_tool_enabled(host, id.trim(), true).await?;
        return Ok(DispatchResult::lines(vec![HistoryLine::agent(
            registry_view::tool_line(&tool),
        )]));
    }
    if let Some(id) = command.strip_prefix("/disable ") {
        let tool = client::set_tool_enabled(host, id.trim(), false).await?;
        return Ok(DispatchResult::lines(vec![HistoryLine::agent(
            registry_view::tool_line(&tool),
        )]));
    }
    if let Some(label) = command.strip_prefix("/graph") {
        return graph_lines(host, label.trim()).await;
    }
    if command == "/security" {
        let settings = client::get_security_settings(host).await?;
        let mut lines = vec![HistoryLine::system(format!(
            "{} allowlisted programs (empty = allow any once approved)",
            settings.process_exec_allowlist.len()
        ))];
        lines.extend(
            settings
                .process_exec_allowlist
                .into_iter()
                .map(HistoryLine::agent),
        );
        return Ok(DispatchResult::lines(lines));
    }
    if let Some(program) = command.strip_prefix("/allow ") {
        let mut settings = client::get_security_settings(host).await?;
        let program = program.trim().to_owned();
        if !settings.process_exec_allowlist.contains(&program) {
            settings.process_exec_allowlist.push(program.clone());
            settings.process_exec_allowlist.sort();
        }
        client::save_security_settings(host, settings.process_exec_allowlist).await?;
        return Ok(DispatchResult::lines(vec![HistoryLine::system(format!(
            "allowed {program}"
        ))]));
    }
    if let Some(program) = command.strip_prefix("/disallow ") {
        let mut settings = client::get_security_settings(host).await?;
        let program = program.trim().to_owned();
        settings
            .process_exec_allowlist
            .retain(|item| item != &program);
        client::save_security_settings(host, settings.process_exec_allowlist).await?;
        return Ok(DispatchResult::lines(vec![HistoryLine::system(format!(
            "disallowed {program}"
        ))]));
    }
    if let Some(path) = command.strip_prefix("/skill-install ") {
        return Ok(DispatchResult::lines(
            interactive_registry::install_skill_lines(host, path).await?,
        ));
    }
    if let Some(raw) = command.strip_prefix("/invoke ") {
        return Ok(DispatchResult::lines(
            interactive_registry::invoke_lines(host, raw).await?,
        ));
    }
    if command == "/tool-history" {
        return Ok(DispatchResult::lines(
            interactive_registry::history_lines(host).await?,
        ));
    }
    if command == "/maintenance" {
        let status = client::maintenance_status(host).await?;
        return Ok(DispatchResult::lines(vec![HistoryLine::system(format!(
            "retention {} days | completed workflows {}",
            status.retention_days, status.max_completed_workflows
        ))]));
    }
    if command == "/prune" {
        let report = client::prune_now(host).await?;
        return Ok(DispatchResult::lines(vec![HistoryLine::system(format!(
            "pruned {} audit rows | {} workflows",
            report.tool_invocations, report.workflows
        ))]));
    }
    if command == "/workflows" {
        let workflows = client::list_workflows(host).await?;
        let mut lines = vec![HistoryLine::system(format!(
            "{} workflows",
            workflows.len()
        ))];
        for workflow in workflows {
            lines.push(HistoryLine::agent(workflow_view::summary_line(&workflow)));
        }
        return Ok(DispatchResult::lines(lines));
    }
    if let Some(id) = command.strip_prefix("/inspect ") {
        let status = client::get_workflow_status(host, id.trim()).await?;
        let lines = workflow_view::status_lines(&status)
            .into_iter()
            .map(HistoryLine::agent)
            .collect();
        return Ok(DispatchResult::lines(lines));
    }
    if let Some(id) = command.strip_prefix("/watch ") {
        let lines = workflow_watch::collect(host, id.trim(), WatchOptions::interactive())
            .await?
            .into_iter()
            .map(HistoryLine::agent)
            .collect();
        return Ok(DispatchResult::lines(lines));
    }
    if let Some(raw) = command.strip_prefix("/logs ") {
        return workflow_log_lines(host, raw).await;
    }
    if let Some(task) = command.strip_prefix("/task ") {
        let status = client::execute_task(host, task).await?;
        return Ok(DispatchResult::lines(vec![HistoryLine::system(status)]));
    }
    if let Some(content) = command.strip_prefix("/remember ") {
        let id = client::store_memory(host, 1, content, Vec::new(), Vec::new()).await?;
        return Ok(DispatchResult::lines(vec![HistoryLine::system(format!(
            "remembered {id}"
        ))]));
    }
    if let Some(raw) = command.strip_prefix("/lesson ") {
        return store_lesson(host, raw).await;
    }
    if let Some(query) = command.strip_prefix("/recall") {
        return memory_lines(host, query.trim(), 0).await;
    }
    if let Some(query) = command.strip_prefix("/dreams") {
        return memory_lines(host, query.trim(), 4).await;
    }
    if let Some(title) = command.strip_prefix("/workflow ") {
        return workflow_lines(host, title).await;
    }
    if let Some(id) = command.strip_prefix("/approve ") {
        let message = client::approve_workflow(host, id.trim(), 2).await?;
        return Ok(DispatchResult::lines(vec![HistoryLine::system(message)]));
    }
    if let Some(id) = command.strip_prefix("/cancel ") {
        let cancelled = client::cancel_workflow(host, id.trim(), "cancelled from tui").await?;
        return Ok(DispatchResult::lines(vec![HistoryLine::system(format!(
            "cancelled={cancelled}"
        ))]));
    }

    let status = client::execute_task(host, command).await?;
    Ok(DispatchResult::lines(vec![HistoryLine::system(status)]))
}

async fn store_lesson(host: &str, raw: &str) -> Result<DispatchResult> {
    let parts = raw
        .split('|')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() != 3 {
        return Ok(DispatchResult::lines(vec![HistoryLine::error(
            "usage: /lesson <scope> | <error> | <correction>".to_owned(),
        )]));
    }
    let content = format!(
        "Mistake in {}: {}. Correction: {}",
        parts[0], parts[1], parts[2]
    );
    let id = client::store_memory(
        host,
        3,
        &content,
        vec![
            ("scope".to_owned(), parts[0].to_owned()),
            ("error_signature".to_owned(), parts[1].to_owned()),
            ("correction".to_owned(), parts[2].to_owned()),
            ("severity".to_owned(), "3".to_owned()),
        ],
        vec![parts[0].to_owned()],
    )
    .await?;
    Ok(DispatchResult::lines(vec![HistoryLine::system(format!(
        "lesson stored {id}"
    ))]))
}

async fn memory_lines(host: &str, query: &str, memory_type: i32) -> Result<DispatchResult> {
    let memories = client::recall_memory(host, query, memory_type, 10).await?;
    let mut lines = vec![HistoryLine::system(format!("{} memories", memories.len()))];
    for memory in memories {
        lines.push(HistoryLine::agent(format!(
            "{} | type {} | {}",
            memory.id,
            memory.memory_type,
            memory.content.replace('\n', " ")
        )));
    }
    Ok(DispatchResult::lines(lines))
}

async fn workflow_lines(host: &str, title: &str) -> Result<DispatchResult> {
    let events = client::start_workflow(host, title, title, "").await?;
    let mut lines = Vec::new();
    for event in events {
        lines.push(HistoryLine::agent(format!(
            "{} | phase {} | {}",
            event.workflow_id, event.phase, event.message
        )));
        if event.requires_approval {
            lines.push(HistoryLine::system(format!(
                "approval required: /approve {}",
                event.workflow_id
            )));
        }
    }
    Ok(DispatchResult::lines(lines))
}

async fn mcp_add_lines(host: &str, raw: &str) -> Result<DispatchResult> {
    let mut parts = raw.split_whitespace();
    let (Some(adapter_id), Some(name), Some(command)) = (parts.next(), parts.next(), parts.next())
    else {
        return Ok(DispatchResult::lines(vec![HistoryLine::error(
            "usage: /mcp-add <id> <name> <command> [args...]".to_owned(),
        )]));
    };
    let arguments: Vec<String> = parts.map(str::to_owned).collect();
    let adapter = client::register_mcp(host, adapter_id, name, command, arguments, "").await?;
    Ok(DispatchResult::lines(vec![HistoryLine::system(format!(
        "registered\t{}",
        adapter.adapter_id
    ))]))
}

async fn graph_lines(host: &str, label: &str) -> Result<DispatchResult> {
    let graph = client::get_knowledge_graph(host, label, 2, 50).await?;
    let mut lines = vec![HistoryLine::system(format!(
        "{} nodes | {} edges",
        graph.nodes.len(),
        graph.edges.len()
    ))];
    for node in &graph.nodes {
        lines.push(HistoryLine::agent(format!(
            "node\t{}\t{}\t{}",
            node.id, node.node_type, node.label
        )));
    }
    for edge in &graph.edges {
        lines.push(HistoryLine::agent(format!(
            "edge\t{} -[{}]-> {}",
            edge.source_id, edge.relationship, edge.target_id
        )));
    }
    Ok(DispatchResult::lines(lines))
}

async fn workflow_log_lines(host: &str, raw: &str) -> Result<DispatchResult> {
    let parts = raw.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 2 {
        return Ok(DispatchResult::lines(vec![HistoryLine::error(
            "usage: /logs <workflow_id> <agent_id>".to_owned(),
        )]));
    }
    let logs = client::get_agent_logs(host, parts[0], parts[1], 20).await?;
    let mut lines = vec![HistoryLine::system(format!("{} log entries", logs.len()))];
    for entry in logs {
        lines.push(HistoryLine::agent(workflow_view::log_line(&entry)));
    }
    Ok(DispatchResult::lines(lines))
}
