/// Single source of truth for every slash command: (name incl. leading '/', short description).
/// Drives both the static `/help` dump and the live autocomplete menu in the composer.
pub const COMMANDS: &[(&str, &str)] = &[
    ("/new", "start a fresh model session"),
    ("/sessions", "list resumable sessions"),
    (
        "/resume",
        "resume a saved session (usage: /resume <session_id>)",
    ),
    (
        "/delete",
        "delete a saved session (usage: /delete <session_id>)",
    ),
    ("/providers", "list configured provider profiles"),
    (
        "/provider",
        "switch provider profile (usage: /provider <profile_id>)",
    ),
    (
        "/provider-remove",
        "remove a provider profile (usage: /provider-remove <profile_id>)",
    ),
    (
        "/model",
        "change the active profile model (usage: /model <model_id>)",
    ),
    ("/ping", "check daemon"),
    ("/status", "compact operator status"),
    ("/doctor", "inspect local runtime health"),
    ("/agents", "list registered agents"),
    ("/tools", "list governed tools and policies"),
    (
        "/enable",
        "enable a governed tool (usage: /enable <tool_id>)",
    ),
    (
        "/disable",
        "disable a governed tool (usage: /disable <tool_id>)",
    ),
    ("/skills", "list skill packs and tool bindings"),
    (
        "/skill-install",
        "install a self-signed skill bundle (usage: /skill-install <bundle.json>)",
    ),
    ("/mcp", "list MCP adapters"),
    (
        "/mcp-add",
        "register an MCP adapter (usage: /mcp-add <id> <name> <command> [args...])",
    ),
    (
        "/mcp-catalog",
        "browse the curated MCP connector catalog (marketplace)",
    ),
    (
        "/mcp-quickadd",
        "register an adapter from the catalog (usage: /mcp-quickadd <catalog_id> <adapter_id> [extra args...])",
    ),
    (
        "/mcp-start",
        "start and discover MCP tools (usage: /mcp-start <id>)",
    ),
    ("/mcp-stop", "stop an MCP adapter (usage: /mcp-stop <id>)"),
    (
        "/mcp-remove",
        "remove an MCP adapter (usage: /mcp-remove <id>)",
    ),
    (
        "/invoke",
        "invoke governed tool (usage: /invoke <tool_id> [--approve] [key=value ...])",
    ),
    ("/tool-history", "show recent tool invocation audit"),
    ("/maintenance", "show retention policy"),
    ("/prune", "run retention now"),
    (
        "/graph",
        "explore the knowledge graph as text (usage: /graph [label])",
    ),
    ("/security", "list the process.exec command allowlist"),
    (
        "/allow",
        "allow a program for process.exec (usage: /allow <program>)",
    ),
    (
        "/disallow",
        "remove a program from the allowlist (usage: /disallow <program>)",
    ),
    (
        "/diff",
        "show uncommitted git changes (usage: /diff [path])",
    ),
    (
        "/branch",
        "create and check out a git branch (usage: /branch <name>)",
    ),
    ("/cron", "list scheduled cron jobs"),
    (
        "/cron-create",
        "schedule a job (usage: /cron-create <name> <tool_id> <interval_seconds> [input_json])",
    ),
    (
        "/cron-enable",
        "enable a cron job (usage: /cron-enable <job_id>)",
    ),
    (
        "/cron-disable",
        "disable a cron job (usage: /cron-disable <job_id>)",
    ),
    (
        "/cron-delete",
        "remove a cron job (usage: /cron-delete <job_id>)",
    ),
    (
        "/cron-run",
        "run a cron job immediately (usage: /cron-run <job_id>)",
    ),
    (
        "/artifacts",
        "list saved artifacts (usage: /artifacts [query])",
    ),
    (
        "/artifact-show",
        "print an artifact's full content (usage: /artifact-show <artifact_id>)",
    ),
    (
        "/artifact-delete",
        "remove an artifact (usage: /artifact-delete <artifact_id>)",
    ),
    ("/workflows", "list workflows"),
    (
        "/workflow",
        "start workflow and stop at approval (usage: /workflow <title>)",
    ),
    (
        "/watch",
        "watch workflow progress (usage: /watch <workflow_id>)",
    ),
    (
        "/inspect",
        "show workflow status and recent events (usage: /inspect <workflow_id>)",
    ),
    (
        "/logs",
        "show agent logs (usage: /logs <workflow_id> <agent_id>)",
    ),
    (
        "/approve",
        "approve with hybrid mode (usage: /approve <workflow_id>)",
    ),
    ("/cancel", "cancel workflow (usage: /cancel <workflow_id>)"),
    ("/memory", "show memory commands"),
    (
        "/remember",
        "store long-term memory (usage: /remember <text>)",
    ),
    (
        "/lesson",
        "store negative lesson (usage: /lesson <scope> | <error> | <correction>)",
    ),
    ("/recall", "recall memory (usage: /recall [query])"),
    ("/dreams", "recall nightly dreams (usage: /dreams [query])"),
    ("/task", "store an explicit task (usage: /task <text>)"),
    (
        "/init",
        "explore the workspace and draft a project-context summary",
    ),
    (
        "/interview",
        "ask clarifying questions before starting work (usage: /interview [topic])",
    ),
    (
        "/deepwork",
        "work autonomously end-to-end on a task (usage: /deepwork <task>)",
    ),
    ("/clear", "clear transcript"),
    ("/quit", "exit the shell"),
    ("/exit", "exit the shell"),
];

const KEY_HINTS: &[&str] = &[
    "plain text - stream a model response with governed tools",
    "Tab - autocomplete the highlighted slash command",
    "Up/Down - command history",
    "Left/Right/Home/End/Delete - edit prompt",
    "PageUp/PageDown - scroll transcript",
    "Ctrl-L - clear transcript",
    "Ctrl-U - clear prompt",
    "Esc or Ctrl-C - cancel active request, otherwise exit",
];

pub fn help_text() -> String {
    let mut lines: Vec<String> = COMMANDS
        .iter()
        .map(|(name, description)| format!("{name} - {description}"))
        .collect();
    lines.extend(KEY_HINTS.iter().map(|line| (*line).to_owned()));
    lines.join("\n")
}

pub fn memory_help_text() -> String {
    [
        "memory commands",
        "/remember <text>",
        "/lesson <scope> | <error> | <correction>",
        "/recall [query]",
        "/dreams [query]",
    ]
    .join("\n")
}

/// Slash commands whose name contains `filter` (case-insensitive), for the live composer
/// autocomplete menu. `filter` excludes the leading '/'. Exact/prefix matches sort first.
pub fn matches(filter: &str) -> Vec<(&'static str, &'static str)> {
    if filter.is_empty() {
        return COMMANDS.to_vec();
    }
    let needle = filter.to_lowercase();
    let mut matched: Vec<(&'static str, &'static str)> = COMMANDS
        .iter()
        .copied()
        .filter(|(name, _)| {
            name.trim_start_matches('/')
                .to_lowercase()
                .contains(&needle)
        })
        .collect();
    matched.sort_by_key(|(name, _)| {
        let stripped = name.trim_start_matches('/').to_lowercase();
        (!stripped.starts_with(&needle), name.len())
    });
    matched
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn matches_prefixes_sort_before_substring_matches() {
        let results = matches("q");
        let names: Vec<&str> = results.iter().map(|(name, _)| *name).collect();
        assert!(names.contains(&"/quit"));
        // "/quit" (prefix match on "q") should rank before "/mcp-remove"-style substring hits.
        let quit_index = names.iter().position(|name| *name == "/quit").unwrap();
        for (index, name) in names.iter().enumerate() {
            if !name.trim_start_matches('/').starts_with('q') {
                assert!(quit_index < index);
            }
        }
    }

    #[test]
    fn matches_is_case_insensitive() {
        assert!(!matches("CRON").is_empty());
    }

    #[test]
    fn empty_filter_returns_every_command() {
        assert_eq!(matches("").len(), super::COMMANDS.len());
    }

    #[test]
    fn unknown_filter_returns_nothing() {
        assert!(matches("zzz-not-a-command").is_empty());
    }
}
