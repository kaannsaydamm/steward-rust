use super::{ShellState, BLUE, BORDER, GOLD, MUTED, PARCHMENT};
use ratatui::layout::{Alignment, Rect};
use ratatui::prelude::{Frame, Line, Modifier, Span, Style};
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn draw(frame: &mut Frame<'_>, area: Rect, state: &ShellState) {
    let online = state.daemon_status.starts_with("online");
    let status_color = if online { BLUE } else { GOLD };
    let session = state.session_id.as_deref().unwrap_or("new session");
    let title = Line::from(vec![
        Span::styled(
            ">_ STEWARD",
            Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  clean-room agent runtime", Style::default().fg(MUTED)),
        Span::styled("  ", Style::default()),
        Span::styled(
            state.daemon_status.to_uppercase(),
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    let model = Line::from(vec![
        Span::styled("MODEL  ", Style::default().fg(MUTED)),
        Span::styled(&state.model, Style::default().fg(PARCHMENT)),
        Span::styled("    PROFILE  ", Style::default().fg(MUTED)),
        Span::styled(&state.active_profile, Style::default().fg(BLUE)),
    ]);
    let details = Line::from(vec![
        Span::styled("SESSION  ", Style::default().fg(MUTED)),
        Span::styled(short(session, 24), Style::default().fg(PARCHMENT)),
        Span::styled("    TOKENS  ", Style::default().fg(MUTED)),
        Span::styled(
            format!(
                "{} in / {} out",
                state.prompt_tokens, state.completion_tokens
            ),
            Style::default().fg(PARCHMENT),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(vec![title, model, details])
            .alignment(Alignment::Left)
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(BORDER)),
            ),
        area,
    );
}

fn short(value: &str, max: usize) -> &str {
    value.get(..max).unwrap_or(value)
}
