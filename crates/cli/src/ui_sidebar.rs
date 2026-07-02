use super::{ShellState, BLUE, BORDER, GOLD, MUTED, PARCHMENT};
use ratatui::layout::Rect;
use ratatui::prelude::{Frame, Line, Modifier, Span, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub fn draw_sessions(frame: &mut Frame<'_>, area: Rect, state: &ShellState) {
    let mut lines = avatar();
    lines.push(Line::from(""));
    lines.push(label("RECENT SESSIONS"));
    if state.sessions.is_empty() {
        lines.push(muted("No saved sessions yet"));
    }
    for session in state.sessions.iter().take(8) {
        let selected = state.session_id.as_deref() == Some(session.session_id.as_str());
        let marker = if selected { ">" } else { " " };
        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(GOLD)),
            Span::styled(
                format!(" {}", truncate(&session.title, 20)),
                Style::default().fg(if selected { PARCHMENT } else { MUTED }),
            ),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(label("QUICK COMMANDS"));
    for command in ["/new", "/sessions", "/providers", "/tools", "/help"] {
        lines.push(Line::from(Span::styled(command, Style::default().fg(BLUE))));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(panel("workspace")),
        area,
    );
}

pub fn draw_activity(frame: &mut Frame<'_>, area: Rect, state: &ShellState) {
    let mut lines = vec![label("TOOL ACTIVITY")];
    if state.tool_activity.is_empty() {
        lines.push(muted("Waiting for a tool call"));
    }
    for item in state.tool_activity.iter().rev().take(7) {
        let color = if item.status == "running" { GOLD } else { BLUE };
        lines.push(Line::from(vec![
            Span::styled("- ", Style::default().fg(color)),
            Span::styled(&item.name, Style::default().fg(PARCHMENT)),
        ]));
        lines.push(Line::from(Span::styled(
            format!("  {}", truncate(&item.detail, 25)),
            Style::default().fg(MUTED),
        )));
    }
    lines.push(Line::from(""));
    lines.push(label("LOCAL SERVICES"));
    lines.push(Line::from(vec![
        Span::styled("RPC  ", Style::default().fg(MUTED)),
        Span::styled(truncate(&state.host, 22), Style::default().fg(PARCHMENT)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("WEB  ", Style::default().fg(MUTED)),
        Span::styled(truncate(&state.web_url, 22), Style::default().fg(PARCHMENT)),
    ]));
    frame.render_widget(Paragraph::new(lines).block(panel("inspector")), area);
}

fn avatar() -> Vec<Line<'static>> {
    [
        "     _",
        "    ( )",
        "   [ - ]",
        "  /     \\",
        " |  ^w^  |",
        " [=======]",
        "   \\___\\",
    ]
    .into_iter()
    .map(|text| Line::from(Span::styled(text, Style::default().fg(GOLD))))
    .collect()
}

fn label(text: &'static str) -> Line<'static> {
    Line::from(Span::styled(
        text,
        Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
    ))
}

fn muted(text: &'static str) -> Line<'static> {
    Line::from(Span::styled(text, Style::default().fg(MUTED)))
}

fn panel(title: &'static str) -> Block<'static> {
    Block::default()
        .title(title)
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(BORDER))
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_owned();
    }
    value
        .chars()
        .take(max.saturating_sub(3))
        .collect::<String>()
        + "..."
}
