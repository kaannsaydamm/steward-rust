use crate::client;
use crate::interactive_commands;
use crate::interactive_help;
use crate::tui::Tui;
use crate::ui::{self, ShellState};
use anyhow::{Context as _, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::Duration;

pub async fn run(host: String) -> Result<()> {
    let mut shell = StewardShell::new(host);
    shell.bootstrap().await;
    let mut tui = Tui::init().context("starting Steward interactive terminal")?;

    loop {
        tui.draw(|frame| ui::render(frame, shell.state()))?;
        if event::poll(Duration::from_millis(100))? {
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
}

impl StewardShell {
    fn new(host: String) -> Self {
        Self {
            host: host.clone(),
            state: ShellState::new(host),
            command_history: Vec::new(),
            history_cursor: None,
            draft_input: String::new(),
        }
    }

    const fn state(&self) -> &ShellState {
        &self.state
    }

    async fn bootstrap(&mut self) {
        match client::ping(&self.host).await {
            Ok(status) => {
                self.state.daemon_status = format!("online: {status}");
                self.state.push_system("daemon connected");
            }
            Err(error) => {
                self.state.daemon_status = "offline".to_owned();
                self.state
                    .push_error(format!("daemon unavailable: {error}"));
            }
        }
        self.state.push_system("type /help for steward commands");
    }

    async fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return Ok(false);
        }
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(true),
            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.clear_history()
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.clear_input()
            }
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
        if command == "/quit" || command == "/exit" {
            return Ok(true);
        }
        if command == "/clear" {
            self.clear_history();
            return Ok(false);
        }
        if command == "/help" {
            self.state.push_system(interactive_help::help_text());
            return Ok(false);
        }
        if command == "/memory" {
            self.state.push_system(interactive_help::memory_help_text());
            return Ok(false);
        }
        self.run_command(&command).await;
        Ok(false)
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
                for line in result.lines {
                    self.state.history.push(line);
                }
                self.state.scroll_offset = 0;
            }
            Err(error) => self.state.push_error(error.to_string()),
        }
    }

    fn insert_char(&mut self, ch: char) {
        self.state.input.insert(self.state.input_cursor, ch);
        self.state.input_cursor += ch.len_utf8();
        self.reset_history_cursor();
    }

    fn backspace(&mut self) {
        if self.state.input_cursor == 0 {
            return;
        }
        self.state.input_cursor = previous_boundary(&self.state.input, self.state.input_cursor);
        self.state.input.remove(self.state.input_cursor);
        self.reset_history_cursor();
    }

    fn delete(&mut self) {
        if self.state.input_cursor < self.state.input.len() {
            self.state.input.remove(self.state.input_cursor);
            self.reset_history_cursor();
        }
    }

    fn move_cursor_left(&mut self) {
        self.state.input_cursor = previous_boundary(&self.state.input, self.state.input_cursor);
    }

    fn move_cursor_right(&mut self) {
        self.state.input_cursor = next_boundary(&self.state.input, self.state.input_cursor);
    }

    fn clear_input(&mut self) {
        self.state.input.clear();
        self.state.input_cursor = 0;
        self.reset_history_cursor();
    }

    fn clear_history(&mut self) {
        self.state.history.clear();
        self.state.scroll_offset = 0;
        self.state.push_system("transcript cleared");
    }

    fn scroll_up(&mut self) {
        self.state.scroll_offset = (self.state.scroll_offset + 10).min(self.state.history.len());
    }

    fn scroll_down(&mut self) {
        self.state.scroll_offset = self.state.scroll_offset.saturating_sub(10);
    }

    fn push_command_history(&mut self, command: String) {
        if self.command_history.last() != Some(&command) {
            self.command_history.push(command);
            if self.command_history.len() > 100 {
                self.command_history.remove(0);
            }
        }
        self.history_cursor = None;
        self.draft_input.clear();
    }

    fn history_prev(&mut self) {
        if self.command_history.is_empty() {
            return;
        }
        let next = match self.history_cursor {
            Some(index) => index.saturating_sub(1),
            None => {
                self.draft_input = self.state.input.clone();
                self.command_history.len().saturating_sub(1)
            }
        };
        self.apply_history_entry(next);
    }

    fn history_next(&mut self) {
        let Some(index) = self.history_cursor else {
            return;
        };
        if index + 1 >= self.command_history.len() {
            self.history_cursor = None;
            self.state.input = self.draft_input.clone();
            self.state.input_cursor = self.state.input.len();
            return;
        }
        self.apply_history_entry(index + 1);
    }

    fn apply_history_entry(&mut self, index: usize) {
        self.history_cursor = Some(index);
        self.state.input = self.command_history[index].clone();
        self.state.input_cursor = self.state.input.len();
    }

    fn reset_history_cursor(&mut self) {
        self.history_cursor = None;
        self.draft_input.clear();
    }
}

fn previous_boundary(input: &str, cursor: usize) -> usize {
    input[..cursor]
        .char_indices()
        .last()
        .map_or(0, |(index, _)| index)
}

fn next_boundary(input: &str, cursor: usize) -> usize {
    input[cursor..]
        .char_indices()
        .nth(1)
        .map_or(input.len(), |(index, _)| cursor + index)
}

#[cfg(test)]
#[path = "interactive_tests.rs"]
mod tests;
