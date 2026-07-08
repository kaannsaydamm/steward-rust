use crate::client;
use crate::client_chat;
use crate::interactive_commands;
use crate::interactive_help;
use crate::prompts;
use crate::tui::Tui;
use crate::ui::{self, ShellState};
use anyhow::{Context as _, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::Duration;
use steward_core::pb::{ChatEvent, ChatEventKind};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;

#[path = "interactive_editor.rs"]
mod editor;
#[path = "interactive_navigation.rs"]
mod navigation;

pub async fn run(host: String, web_url: String) -> Result<()> {
    let mut shell = StewardShell::new(host);
    shell.bootstrap(&web_url).await;
    let mut tui = Tui::init().context("starting Steward interactive terminal")?;
    loop {
        shell.state.tick = shell.state.tick.wrapping_add(1);
        if shell.drain_chat_events() {
            shell.refresh_sessions().await;
        }
        tui.draw(|frame| ui::render(frame, shell.state()))?;
        if event::poll(Duration::from_millis(80))? {
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if shell.handle_key(key).await? {
                break;
            }
        }
    }
    Ok(())
}

struct StewardShell {
    host: String,
    state: ShellState,
    command_history: Vec<String>,
    history_cursor: Option<usize>,
    draft_input: String,
    chat_tx: UnboundedSender<Result<ChatEvent, String>>,
    chat_rx: UnboundedReceiver<Result<ChatEvent, String>>,
    chat_task: Option<JoinHandle<()>>,
}

impl StewardShell {
    fn new(host: String) -> Self {
        let (chat_tx, chat_rx) = mpsc::unbounded_channel();
        Self {
            host: host.clone(),
            state: ShellState::new(host),
            command_history: Vec::new(),
            history_cursor: None,
            draft_input: String::new(),
            chat_tx,
            chat_rx,
            chat_task: None,
        }
    }

    const fn state(&self) -> &ShellState {
        &self.state
    }

    async fn bootstrap(&mut self, web_url: &str) {
        self.state.web_url = web_url.to_owned();
        match client::ping(&self.host).await {
            Ok(status) => {
                self.state.daemon_status = format!("online: {status}");
                self.state
                    .push_system(format!("Web UI is running at {web_url}"));
            }
            Err(error) => {
                self.state.daemon_status = "offline".to_owned();
                self.state
                    .push_error(format!("daemon unavailable: {error}"));
                return;
            }
        }
        self.refresh_provider().await;
        self.refresh_sessions().await;
        self.push_capability_summary().await;
        self.state
            .push_system("Type /help for commands. Plain text starts a model session.");
    }

    /// One-line capability inventory on startup ("N tools · M skills · K MCP adapters"),
    /// matching the landing summary Hermes and comparable agent CLIs print so the operator
    /// can see what the daemon is armed with before typing anything.
    async fn push_capability_summary(&mut self) {
        let (tools, skills, adapters) = tokio::join!(
            client::list_tools(&self.host),
            client::list_skills(&self.host),
            client::list_mcp(&self.host),
        );
        let (Ok(tools), Ok(skills), Ok(adapters)) = (tools, skills, adapters) else {
            return;
        };
        let enabled = tools.iter().filter(|tool| tool.enabled).count();
        let running = adapters
            .iter()
            .filter(|adapter| adapter.status == "running")
            .count();
        let model = if self.state.model.is_empty() {
            "no active model".to_owned()
        } else {
            self.state.model.clone()
        };
        self.state.push_system(format!(
            "{} tools ({enabled} enabled) · {} skills · {} MCP adapters ({running} running) · {model}",
            tools.len(),
            skills.len(),
            adapters.len(),
        ));
    }

    async fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return Ok(false);
        }
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.state.busy {
                    self.cancel_chat();
                } else {
                    return Ok(true);
                }
            }
            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.clear_history()
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.clear_input()
            }
            KeyCode::Esc if self.state.busy => self.cancel_chat(),
            KeyCode::Esc => return Ok(true),
            KeyCode::Enter => return self.submit().await,
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete(),
            KeyCode::Left => self.move_cursor_left(),
            KeyCode::Right => self.move_cursor_right(),
            KeyCode::Home => self.state.input_cursor = 0,
            KeyCode::End => self.state.input_cursor = self.state.input.len(),
            KeyCode::Up => self.history_prev(),
            KeyCode::Down => self.history_next(),
            KeyCode::PageUp => self.scroll_up(),
            KeyCode::PageDown => self.scroll_down(),
            KeyCode::Tab => self.autocomplete_command(),
            KeyCode::Char(ch) => self.insert_char(ch),
            _ => {}
        }
        Ok(false)
    }

    async fn submit(&mut self) -> Result<bool> {
        let command = self.state.input.trim().to_owned();
        self.clear_input();
        if command.is_empty() {
            return Ok(false);
        }
        self.push_command_history(command.clone());
        self.state.push_user(command.clone());
        match command.as_str() {
            "/quit" | "/exit" => return Ok(true),
            "/clear" => self.clear_history(),
            "/help" => self.state.push_system(interactive_help::help_text()),
            "/memory" => self.state.push_system(interactive_help::memory_help_text()),
            "/new" => self.start_new_session(),
            "/sessions" => self.show_sessions().await,
            "/providers" => self.show_providers().await,
            _ => self.route_command(&command).await,
        }
        Ok(false)
    }

    async fn route_command(&mut self, command: &str) {
        if let Some(id) = command.strip_prefix("/resume ") {
            self.resume_session(id.trim()).await;
        } else if let Some(id) = command.strip_prefix("/delete ") {
            self.delete_session(id.trim()).await;
        } else if let Some(id) = command.strip_prefix("/provider-remove ") {
            self.remove_provider(id.trim()).await;
        } else if let Some(id) = command.strip_prefix("/provider ") {
            self.activate_provider(id.trim()).await;
        } else if let Some(model) = command.strip_prefix("/model ") {
            self.change_model(model.trim()).await;
        } else if let Some(message) = command.strip_prefix("/task ") {
            self.start_chat(message.trim());
        } else if command == "/compact" {
            self.compact_session().await;
        } else if command == "/context" {
            self.show_context().await;
        } else if command == "/init" {
            self.start_chat(prompts::INIT);
        } else if command == "/interview" {
            self.start_chat(&prompts::interview(""));
        } else if let Some(topic) = command.strip_prefix("/interview ") {
            self.start_chat(&prompts::interview(topic.trim()));
        } else if let Some(task) = command.strip_prefix("/deepwork ") {
            self.start_chat(&prompts::deepwork(task.trim()));
        } else if command.starts_with('/') {
            self.run_command(command).await;
        } else {
            self.start_chat(command);
        }
    }

    /// Frees context by replacing this session's stored history with a model-written summary
    /// (Claude Code's /compact). Runs inline rather than through the chat stream because it
    /// is a unary RPC, not a conversational turn.
    async fn compact_session(&mut self) {
        let Some(session_id) = self.state.session_id.clone() else {
            self.state
                .push_error("No active session to compact. Send a message first.");
            return;
        };
        self.state.busy = true;
        let result = client_chat::compact_session(&self.host, &session_id).await;
        self.state.busy = false;
        match result {
            Ok(response) => {
                self.state.push_system(format!(
                    "compacted: {} messages replaced with a summary",
                    response.removed_messages
                ));
                self.state.history.push(ui::HistoryLine::agent(format!(
                    "Summary:\n{}",
                    response.summary
                )));
                self.state.scroll_offset = 0;
            }
            Err(error) => self.state.push_error(format!("compact failed: {error:#}")),
        }
    }

    /// Visualizes what is filling the active session's context: per-role message counts and
    /// estimated token share rendered as proportional bars (tokens are estimated at ~4 chars
    /// each — providers don't expose exact per-message counts, and the estimate is labeled).
    async fn show_context(&mut self) {
        let Some(session_id) = self.state.session_id.clone() else {
            self.state
                .push_error("No active session. Send a message first.");
            return;
        };
        let session = match client_chat::session(&self.host, &session_id).await {
            Ok(session) => session,
            Err(error) => {
                self.state.push_error(format!("context failed: {error:#}"));
                return;
            }
        };
        let mut totals: Vec<(&str, usize, usize)> = vec![
            ("system", 0, 0),
            ("user", 0, 0),
            ("assistant", 0, 0),
            ("tool", 0, 0),
        ];
        for message in &session.messages {
            if let Some(entry) = totals.iter_mut().find(|(role, _, _)| *role == message.role) {
                entry.1 += 1;
                entry.2 += message.content.chars().count();
            }
        }
        let total_chars: usize = totals.iter().map(|(_, _, chars)| chars).sum();
        self.state.push_system(format!(
            "context: {} messages, ~{} tokens estimated (chars/4)",
            session.messages.len(),
            total_chars / 4
        ));
        for (role, count, chars) in totals {
            if count == 0 {
                continue;
            }
            let share = if total_chars == 0 {
                0
            } else {
                (chars * 30).div_ceil(total_chars)
            };
            self.state.push_system(format!(
                "{role:<9} {count:>3} msg  ~{:>6} tok  {}{}",
                chars / 4,
                "█".repeat(share.min(30)),
                "░".repeat(30usize.saturating_sub(share)),
            ));
        }
    }

    fn start_chat(&mut self, message: &str) {
        if self.state.busy {
            self.state
                .push_error("A request is already running. Press Ctrl+C to cancel it.");
            return;
        }
        let host = self.host.clone();
        let session_id = self.state.session_id.clone().unwrap_or_default();
        let message = message.to_owned();
        let sender = self.chat_tx.clone();
        self.state.busy = true;
        self.chat_task = Some(tokio::spawn(async move {
            if let Err(error) =
                client_chat::stream_chat(&host, &session_id, &message, true, sender.clone()).await
            {
                let _ = sender.send(Err(error.to_string()));
            }
        }));
    }

    fn drain_chat_events(&mut self) -> bool {
        let mut completed = false;
        while let Ok(item) = self.chat_rx.try_recv() {
            match item {
                Ok(event) => {
                    completed |= event.kind == ChatEventKind::Done as i32;
                    self.state.apply_chat_event(event);
                }
                Err(error) => {
                    self.state.busy = false;
                    self.state.push_error(error);
                    completed = true;
                }
            }
        }
        if completed {
            self.chat_task = None;
        }
        completed
    }

    fn cancel_chat(&mut self) {
        if let Some(task) = self.chat_task.take() {
            task.abort();
        }
        self.state.busy = false;
        self.state.push_system("request cancelled");
    }

    async fn run_command(&mut self, command: &str) {
        self.state.busy = true;
        let result = interactive_commands::dispatch(&self.host, command).await;
        self.state.busy = false;
        match result {
            Ok(result) => {
                if let Some(status) = result.daemon_status {
                    self.state.daemon_status = status;
                }
                self.state.history.extend(result.lines);
                self.state.scroll_offset = 0;
            }
            Err(error) => self.state.push_error(error.to_string()),
        }
    }
}

#[cfg(test)]
#[path = "interactive_tests.rs"]
mod tests;
