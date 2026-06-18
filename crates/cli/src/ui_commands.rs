use super::{border, line, GOLD, MUTED};
use ratatui::prelude::{Frame, Rect};
use ratatui::widgets::{Paragraph, Wrap};

pub(super) fn draw(frame: &mut Frame<'_>, area: Rect) {
    let commands = Paragraph::new(vec![
        line(GOLD, "COMMANDS"),
        line(MUTED, "/ping"),
        line(MUTED, "/status"),
        line(MUTED, "/doctor"),
        line(MUTED, "/agents"),
        line(MUTED, "/tools"),
        line(MUTED, "/skills"),
        line(MUTED, "/skill-install <bundle>"),
        line(MUTED, "/mcp | /mcp-start <id>"),
        line(MUTED, "/mcp-stop <id>"),
        line(MUTED, "/invoke <tool>"),
        line(MUTED, "/tool-history"),
        line(MUTED, "/workflows"),
        line(MUTED, "/workflow <title>"),
        line(MUTED, "/watch <id>"),
        line(MUTED, "/inspect <id>"),
        line(MUTED, "/logs <id> <agent>"),
        line(MUTED, "/approve <id>"),
        line(MUTED, "/cancel <id>"),
        line(MUTED, "/memory"),
        line(MUTED, "/remember <text>"),
        line(MUTED, "/recall [query]"),
        line(MUTED, "/dreams [query]"),
        line(MUTED, "/task <text>"),
        line(MUTED, "/clear"),
        line(MUTED, "/help"),
        line(GOLD, "KEYS"),
        line(MUTED, "Up/Down history"),
        line(MUTED, "PgUp/PgDn scroll"),
        line(MUTED, "Ctrl-L clear"),
        line(MUTED, "Esc / Ctrl-C"),
    ])
    .block(border("rituals"))
    .wrap(Wrap { trim: true });
    frame.render_widget(commands, area);
}
