use super::{ShellState, BORDER, GOLD, MUTED, PARCHMENT};
use ratatui::layout::{Alignment, Rect};
use ratatui::prelude::{Frame, Line, Modifier, Span, Style};
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn draw(frame: &mut Frame<'_>, area: Rect, state: &ShellState) {
    let cursor = state.input_cursor.min(state.input.len());
    let before = &state.input[..cursor];
    let after = &state.input[cursor..];
    let title = if state.busy {
        "agent is working"
    } else {
        "message steward"
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("> ", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
            Span::styled(before, Style::default().fg(PARCHMENT)),
            Span::styled(" ", Style::default().bg(GOLD)),
            Span::styled(after, Style::default().fg(PARCHMENT)),
        ]))
        .block(
            Block::default()
                .title(title)
                .borders(Borders::TOP | Borders::BOTTOM)
                .border_style(Style::default().fg(BORDER)),
        ),
        area,
    );
}

pub fn draw_footer(frame: &mut Frame<'_>, area: Rect, state: &ShellState) {
    let hint = if state.busy {
        "Ctrl+C cancel  |  PgUp/PgDn scroll  |  /help commands"
    } else {
        "Enter send  |  Tab complete  |  @paths  |  Ctrl+X switch session (web)"
    };
    // prime-agent footer split: daemon-computed state on the left ("—" for
    // unknowns), interaction hints on the right.
    let session = match &state.session_id {
        Some(id) => id.chars().take(8).collect(),
        None => "—".to_owned(),
    };
    let model = if state.model.is_empty() {
        "—".to_owned()
    } else {
        state.model.clone()
    };
    let left = format!("{model} · {session} · {daemon}", daemon = state.daemon_status);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(left, Style::default().fg(GOLD)),
            Span::raw("  "),
            Span::styled(hint, Style::default().fg(MUTED)),
        ]))
        .alignment(Alignment::Left),
        area,
    );
}
