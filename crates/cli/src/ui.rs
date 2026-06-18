use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Color, Frame, Line, Modifier, Span, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

#[path = "ui_commands.rs"]
mod commands;

const GOLD: Color = Color::Rgb(212, 175, 55);
const PARCHMENT: Color = Color::Rgb(232, 226, 210);
const MUTED: Color = Color::Rgb(153, 144, 124);
const ERROR: Color = Color::Rgb(255, 180, 171);

#[derive(Clone, Debug)]
pub enum LineKind {
    User,
    System,
    Agent,
    Error,
}

#[derive(Clone, Debug)]
pub struct HistoryLine {
    pub kind: LineKind,
    pub text: String,
}

impl HistoryLine {
    pub fn user(text: String) -> Self {
        Self {
            kind: LineKind::User,
            text,
        }
    }

    pub fn system(text: String) -> Self {
        Self {
            kind: LineKind::System,
            text,
        }
    }

    pub fn agent(text: String) -> Self {
        Self {
            kind: LineKind::Agent,
            text,
        }
    }

    pub fn error(text: String) -> Self {
        Self {
            kind: LineKind::Error,
            text,
        }
    }
}

#[derive(Debug)]
pub struct ShellState {
    pub host: String,
    pub daemon_status: String,
    pub input: String,
    pub input_cursor: usize,
    pub scroll_offset: usize,
    pub history: Vec<HistoryLine>,
    pub busy: bool,
}

impl ShellState {
    pub fn new(host: String) -> Self {
        Self {
            host,
            daemon_status: "checking".to_owned(),
            input: String::new(),
            input_cursor: 0,
            scroll_offset: 0,
            history: Vec::new(),
            busy: false,
        }
    }

    pub fn push_user(&mut self, text: String) {
        self.history.push(HistoryLine::user(text));
        self.scroll_offset = 0;
        self.truncate_history();
    }

    pub fn push_system<T: Into<String>>(&mut self, text: T) {
        self.history.push(HistoryLine::system(text.into()));
        self.scroll_offset = 0;
        self.truncate_history();
    }

    pub fn push_error<T: Into<String>>(&mut self, text: T) {
        self.history.push(HistoryLine::error(text.into()));
        self.scroll_offset = 0;
        self.truncate_history();
    }

    fn truncate_history(&mut self) {
        if self.history.len() > 200 {
            let drain_to = self.history.len().saturating_sub(200);
            self.history.drain(0..drain_to);
        }
    }
}

pub fn render(frame: &mut Frame<'_>, state: &ShellState) {
    let area = frame.area();
    frame.render_widget(Clear, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),
            Constraint::Min(8),
            Constraint::Length(5),
        ])
        .split(area);

    draw_header(frame, rows[0], state);
    draw_body(frame, rows[1], state);
    draw_prompt(frame, rows[2], state);
}

fn draw_header(frame: &mut Frame<'_>, area: Rect, state: &ShellState) {
    let title = Paragraph::new(vec![
        Line::from(Span::styled(
            " __  __  ____  ____  __  __  ____  ____ ",
            Style::default().fg(GOLD),
        )),
        Line::from(Span::styled(
            "|  \\/  || ___||  _ \\|  \\/  || ___|/ ___|",
            Style::default().fg(GOLD),
        )),
        Line::from(Span::styled(
            "| |\\/| || _|  | |_) | |\\/| || _|  \\___ \\",
            Style::default().fg(GOLD),
        )),
        Line::from(Span::styled(
            "|_|  |_||____||____/|_|  |_||____||____/",
            Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "local-first agent harness - hermes operator shell",
            Style::default().fg(MUTED),
        )),
        Line::from(Span::styled(
            format!("[{}]  {}", state.daemon_status, state.host),
            Style::default().fg(PARCHMENT),
        )),
    ])
    .alignment(Alignment::Center)
    .block(border("gateway"));
    frame.render_widget(title, area);
}

fn draw_body(frame: &mut Frame<'_>, area: Rect, state: &ShellState) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(28), Constraint::Min(20)])
        .split(area);

    commands::draw(frame, cols[0]);
    draw_history(frame, cols[1], state);
}

fn draw_history(frame: &mut Frame<'_>, area: Rect, state: &ShellState) {
    let mut lines = Vec::new();
    let visible_end = state.history.len().saturating_sub(state.scroll_offset);
    let visible_start = visible_end.saturating_sub(80);
    for entry in state.history[visible_start..visible_end].iter() {
        let (prefix, color) = match entry.kind {
            LineKind::User => ("YOU", GOLD),
            LineKind::System => ("SYS", PARCHMENT),
            LineKind::Agent => ("AGENT", Color::Rgb(191, 205, 255)),
            LineKind::Error => ("ERR", ERROR),
        };
        lines.push(Line::from(vec![
            Span::styled(format!("[{prefix}] "), Style::default().fg(color)),
            Span::styled(entry.text.clone(), Style::default().fg(PARCHMENT)),
        ]));
    }
    if state.busy {
        lines.push(Line::from(Span::styled(
            "[WORKING] daemon invocation in progress",
            Style::default().fg(GOLD),
        )));
    }
    let transcript = Paragraph::new(lines)
        .block(border("transcript"))
        .wrap(Wrap { trim: false });
    frame.render_widget(transcript, area);
}

fn draw_prompt(frame: &mut Frame<'_>, area: Rect, state: &ShellState) {
    let cursor = state.input_cursor.min(state.input.len());
    let before = &state.input[..cursor];
    let after = &state.input[cursor..];
    let input = Paragraph::new(Line::from(vec![
        Span::styled("* ", Style::default().fg(GOLD)),
        Span::styled(before, Style::default().fg(PARCHMENT)),
        Span::styled("_", Style::default().fg(GOLD).add_modifier(Modifier::BOLD)),
        Span::styled(after, Style::default().fg(PARCHMENT)),
    ]))
    .block(border("what should we tackle?"));
    frame.render_widget(input, area);
}

fn border(title: &'static str) -> Block<'static> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(77, 70, 53)))
}

fn line(color: Color, text: &'static str) -> Line<'static> {
    Line::from(Span::styled(text, Style::default().fg(color)))
}
