pub fn help_text() -> String {
    [
        "/ping - check daemon",
        "/status - compact operator status",
        "/doctor - inspect local runtime health",
        "/agents - list registered agents",
        "/tools - list governed tools and policies",
        "/skills - list skill packs and tool bindings",
        "/skill-install <bundle.json> - install a self-signed skill bundle",
        "/invoke <tool_id> [--approve] [key=value ...] - invoke governed tool",
        "/tool-history - show recent tool invocation audit",
        "/workflows - list workflows",
        "/workflow <title> - start workflow and stop at approval",
        "/watch <workflow_id> - watch workflow progress",
        "/inspect <workflow_id> - show workflow status and recent events",
        "/logs <workflow_id> <agent_id> - show agent logs",
        "/approve <workflow_id> - approve with hybrid mode",
        "/cancel <workflow_id> - cancel workflow",
        "/memory - show memory commands",
        "/remember <text> - store long-term memory",
        "/lesson <scope> | <error> | <correction> - store negative lesson",
        "/recall [query] - recall memory",
        "/dreams [query] - recall nightly dreams",
        "/task <text> - store an explicit task",
        "/clear - clear transcript",
        "plain text - send as task prompt",
        "Up/Down - command history",
        "Left/Right/Home/End/Delete - edit prompt",
        "PageUp/PageDown - scroll transcript",
        "Ctrl-L - clear transcript",
        "Ctrl-U - clear prompt",
        "Esc or Ctrl-C - exit",
    ]
    .join("\n")
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
