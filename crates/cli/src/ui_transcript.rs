use super::sidebar;
use super::{LineKind, ShellState, BLUE, BORDER, ERROR, GOLD, MUTED, PARCHMENT};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Frame, Line, Modifier, Span, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub fn draw_workspace(frame: &mut Frame<'_>, area: Rect, state: &ShellState) {
    match area.width {
        0..=89 => draw(frame, area, state),
        90..=129 => {
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(27), Constraint::Min(40)])
                .split(area);
            sidebar::draw_sessions(frame, cols[0], state);
            draw(frame, cols[1], state);
        }
        _ => {
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(27),
                    Constraint::Min(55),
                    Constraint::Length(30),
                ])
                .split(area);
            sidebar::draw_sessions(frame, cols[0], state);
            draw(frame, cols[1], state);
            sidebar::draw_activity(frame, cols[2], state);
        }
    }
}

fn draw(frame: &mut Frame<'_>, area: Rect, state: &ShellState) {
    let visible_end = state.history.len().saturating_sub(state.scroll_offset);
    let visible_start = visible_end.saturating_sub((area.height as usize).saturating_mul(3));
    let mut lines = Vec::new();
    for entry in &state.history[visible_start..visible_end] {
        let (role, color) = match entry.kind {
            LineKind::User => ("YOU", GOLD),
            LineKind::System => ("SYSTEM", MUTED),
            LineKind::Agent => ("STEWARD", BLUE),
            LineKind::Tool => ("TOOL", Color::Rgb(137, 205, 190)),
            LineKind::Error => ("ERROR", ERROR),
        };
        lines.push(Line::from(Span::styled(
            role,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )));
        for content in entry.text.lines() {
            lines.push(Line::from(Span::styled(
                content.to_owned(),
                Style::default().fg(PARCHMENT),
            )));
        }
        lines.push(Line::from(""));
    }
    if state.busy {
        let frames = ["thinking .", "thinking ..", "thinking ..."];
        lines.push(Line::from(Span::styled(
            frames[(state.tick as usize / 4) % frames.len()],
            Style::default().fg(GOLD),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::default()
                .title("conversation")
                .borders(Borders::LEFT | Borders::RIGHT)
                .border_style(Style::default().fg(BORDER)),
        ),
        area,
    );
}

use ratatui::prelude::Color;
