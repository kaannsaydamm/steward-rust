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
        self.state
            .push_system("Type /help for commands. Plain text starts a model session.");
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
