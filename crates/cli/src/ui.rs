use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::prelude::{Color, Frame};
use ratatui::widgets::Clear;
use steward_core::pb::{ChatEvent, ChatEventKind, ChatSessionSummary};

#[path = "ui_composer.rs"]
mod composer;
#[path = "ui_header.rs"]
mod header;
#[path = "ui_sidebar.rs"]
mod sidebar;
#[path = "ui_transcript.rs"]
mod transcript;

pub const GOLD: Color = Color::Rgb(226, 186, 77);
pub const PARCHMENT: Color = Color::Rgb(232, 226, 210);
pub const MUTED: Color = Color::Rgb(145, 138, 122);
pub const BORDER: Color = Color::Rgb(75, 71, 61);
pub const BLUE: Color = Color::Rgb(139, 180, 255);
pub const ERROR: Color = Color::Rgb(255, 160, 153);

#[derive(Clone, Debug)]
pub enum LineKind {
    User,
    System,
    Agent,
    Tool,
    Error,
}

#[derive(Clone, Debug)]
pub struct HistoryLine {
    pub kind: LineKind,
    pub text: String,
}

impl HistoryLine {
    fn new(kind: LineKind, text: String) -> Self {
        Self { kind, text }
    }

    pub fn user(text: String) -> Self {
        Self::new(LineKind::User, text)
    }

    pub fn system(text: String) -> Self {
        Self::new(LineKind::System, text)
    }

    pub fn agent(text: String) -> Self {
        Self::new(LineKind::Agent, text)
    }

    pub fn tool(text: String) -> Self {
        Self::new(LineKind::Tool, text)
    }

    pub fn error(text: String) -> Self {
        Self::new(LineKind::Error, text)
    }
}

#[derive(Clone, Debug)]
pub struct ToolActivity {
    pub name: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug)]
pub struct ShellState {
    pub host: String,
    pub web_url: String,
    pub daemon_status: String,
    pub active_profile: String,
    pub model: String,
    pub session_id: Option<String>,
    pub sessions: Vec<ChatSessionSummary>,
    pub tool_activity: Vec<ToolActivity>,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub input: String,
    pub input_cursor: usize,
    pub scroll_offset: usize,
    pub history: Vec<HistoryLine>,
    pub busy: bool,
    pub tick: u64,
}

impl ShellState {
    pub fn new(host: String) -> Self {
        Self {
            host,
            web_url: String::new(),
            daemon_status: "checking".to_owned(),
            active_profile: "not configured".to_owned(),
            model: "-".to_owned(),
            session_id: None,
            sessions: Vec::new(),
            tool_activity: Vec::new(),
            prompt_tokens: 0,
            completion_tokens: 0,
            input: String::new(),
            input_cursor: 0,
            scroll_offset: 0,
            history: Vec::new(),
            busy: false,
            tick: 0,
        }
    }

    pub fn push_user(&mut self, text: String) {
        self.push(HistoryLine::user(text));
    }

    pub fn push_system<T: Into<String>>(&mut self, text: T) {
        self.push(HistoryLine::system(text.into()));
    }

    pub fn push_error<T: Into<String>>(&mut self, text: T) {
        self.push(HistoryLine::error(text.into()));
    }

    pub fn apply_chat_event(&mut self, event: ChatEvent) {
        match ChatEventKind::try_from(event.kind).unwrap_or(ChatEventKind::Unspecified) {
            ChatEventKind::Session => self.session_id = Some(event.session_id),
            ChatEventKind::Text => self.push(HistoryLine::agent(event.content)),
            ChatEventKind::ToolStart => self.record_tool(event, "running"),
            ChatEventKind::ToolResult => self.record_tool(event, "complete"),
            ChatEventKind::Done => {
                self.prompt_tokens += event.prompt_tokens;
                self.completion_tokens += event.completion_tokens;
                self.busy = false;
            }
            ChatEventKind::Unspecified => {}
        }
    }

    fn record_tool(&mut self, event: ChatEvent, status: &str) {
        let activity = ToolActivity {
            name: event.tool_name.clone(),
            status: status.to_owned(),
            detail: event.content.clone(),
        };
        if let Some(existing) = self
            .tool_activity
            .iter_mut()
            .rev()
            .find(|item| item.name == activity.name && item.status == "running")
        {
            *existing = activity;
        } else {
            self.tool_activity.push(activity);
        }
        if self.tool_activity.len() > 12 {
            self.tool_activity.remove(0);
        }
        self.push(HistoryLine::tool(format!(
            "{}  {}",
            event.tool_name, event.content
        )));
    }

    fn push(&mut self, line: HistoryLine) {
        self.history.push(line);
        self.scroll_offset = 0;
        if self.history.len() > 500 {
            let drain_to = self.history.len() - 500;
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
            Constraint::Length(5),
            Constraint::Min(6),
            Constraint::Length(4),
            Constraint::Length(1),
        ])
        .split(area);
    header::draw(frame, rows[0], state);
    transcript::draw_workspace(frame, rows[1], state);
    composer::draw(frame, rows[2], state);
    composer::draw_footer(frame, rows[3], state);
}

#[cfg(test)]
#[path = "ui_tests.rs"]
mod tests;
