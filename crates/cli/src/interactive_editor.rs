use super::StewardShell;
use crate::ui::command_menu;

impl StewardShell {
    pub(super) fn autocomplete_command(&mut self) {
        let Some(filter) = command_menu::active_filter(&self.state.input) else {
            return;
        };
        let Some(best) = command_menu::best_match(filter) else {
            return;
        };
        self.state.input = format!("{best} ");
        self.state.input_cursor = self.state.input.len();
        self.reset_history_cursor();
    }

    pub(super) fn insert_char(&mut self, ch: char) {
        self.state.input.insert(self.state.input_cursor, ch);
        self.state.input_cursor += ch.len_utf8();
        self.reset_history_cursor();
    }

    pub(super) fn backspace(&mut self) {
        if self.state.input_cursor == 0 {
            return;
        }
        self.state.input_cursor = previous_boundary(&self.state.input, self.state.input_cursor);
        self.state.input.remove(self.state.input_cursor);
        self.reset_history_cursor();
    }

    pub(super) fn delete(&mut self) {
        if self.state.input_cursor < self.state.input.len() {
            self.state.input.remove(self.state.input_cursor);
            self.reset_history_cursor();
        }
    }

    pub(super) fn move_cursor_left(&mut self) {
        self.state.input_cursor = previous_boundary(&self.state.input, self.state.input_cursor);
    }

    pub(super) fn move_cursor_right(&mut self) {
        self.state.input_cursor = next_boundary(&self.state.input, self.state.input_cursor);
    }

    pub(super) fn clear_input(&mut self) {
        self.state.input.clear();
        self.state.input_cursor = 0;
        self.reset_history_cursor();
    }

    pub(super) fn clear_history(&mut self) {
        self.state.history.clear();
        self.state.scroll_offset = 0;
        self.state.push_system("transcript cleared");
    }

    pub(super) fn scroll_up(&mut self) {
        self.state.scroll_offset = (self.state.scroll_offset + 10).min(self.state.history.len());
    }

    pub(super) fn scroll_down(&mut self) {
        self.state.scroll_offset = self.state.scroll_offset.saturating_sub(10);
    }

    pub(super) fn push_command_history(&mut self, command: String) {
        if self.command_history.last() != Some(&command) {
            self.command_history.push(command);
            if self.command_history.len() > 100 {
                self.command_history.remove(0);
            }
        }
        self.history_cursor = None;
        self.draft_input.clear();
    }

    pub(super) fn history_prev(&mut self) {
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

    pub(super) fn history_next(&mut self) {
        let Some(index) = self.history_cursor else {
            return;
        };
        if index + 1 >= self.command_history.len() {
            self.history_cursor = None;
            self.state.input = self.draft_input.clone();
            self.state.input_cursor = self.state.input.len();
        } else {
            self.apply_history_entry(index + 1);
        }
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
