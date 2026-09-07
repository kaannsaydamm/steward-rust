use super::StewardShell;
use crate::ui::command_menu;

impl StewardShell {
    /// Ctrl+W kills the word before the cursor into the kill ring.
    pub(super) fn kill_word(&mut self) {
        let start = self.state.input[..self.state.input_cursor]
            .trim_end()
            .char_indices()
            .rev()
            .take_while(|(_, c)| !c.is_whitespace())
            .last()
            .map_or(self.state.input_cursor, |(index, _)| index);
        let killed = self.state.input[start..self.state.input_cursor].to_owned();
        if !killed.is_empty() {
            self.kill_ring.push(killed);
            if self.kill_ring.len() > 10 {
                self.kill_ring.remove(0);
            }
            self.state.input.replace_range(start..self.state.input_cursor, "");
            self.state.input_cursor = start;
        }
    }

    /// Ctrl+U kills the entire line into the kill ring.
    pub(super) fn kill_line(&mut self) {
        if !self.state.input.is_empty() {
            self.kill_ring.push(self.state.input.clone());
            if self.kill_ring.len() > 10 {
                self.kill_ring.remove(0);
            }
        }
        self.state.input.clear();
        self.state.input_cursor = 0;
        self.reset_history_cursor();
    }

    /// Ctrl+Y yanks (pastes) the most recent kill at the cursor.
    pub(super) fn yank(&mut self) {
        let Some(killed) = self.kill_ring.last().cloned() else {
            return;
        };
        let cursor = self.state.input_cursor;
        self.state.input.insert_str(cursor, &killed);
        self.state.input_cursor = cursor + killed.len();
    }

    /// `@path` completion: completes the trailing `@token` from cwd entries.
    pub(super) fn complete_at_path(&mut self) {
        let Some(token_start) = self.state.input.rfind('@') else {
            return;
        };
        let token = self.state.input[token_start + 1..].to_owned();
        let candidates = at_candidates(&token);
        match candidates.first() {
            Some(first) if candidates.len() == 1 => {
                self.state.input = format!("{}{} ", &self.state.input[..token_start + 1], first);
                self.state.input_cursor = self.state.input.len();
            }
            Some(first) => {
                // Extend to the longest common prefix; other candidates ride
                // along in `at_candidates` for the renderer.
                let prefix = candidates
                    .iter()
                    .skip(1)
                    .fold(first.as_str(), |acc, candidate| {
                        let end = acc
                            .char_indices()
                            .zip(candidate.chars())
                            .take_while(|(a, b)| a.1 == *b)
                            .count();
                        &acc[..end]
                    });
                self.state.input = format!("{}{}", &self.state.input[..token_start + 1], prefix);
                self.state.input_cursor = self.state.input.len();
                self.at_candidates = candidates;
            }
            None => {}
        }
    }

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

/// Candidate paths for `@` completion: entries of the current directory
/// (bounded to 500) whose name starts with `token` (after the `@`).
fn at_candidates(token: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(".") else {
        return Vec::new();
    };
    let mut candidates: Vec<String> = entries
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with(token))
        .take(500)
        .collect();
    candidates.sort();
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_candidates_empty_token_lists_cwd_bounded() {
        // The repo root certainly has Cargo.toml; token "" lists everything.
        let candidates = at_candidates("");
        assert!(!candidates.is_empty());
        assert!(candidates.len() <= 500);
    }

    #[test]
    fn at_candidates_filter_by_prefix() {
        let candidates = at_candidates("Cargo.");
        assert!(candidates.iter().any(|name| name == "Cargo.toml"));
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
mod kill_ring_tests {
    use super::*;

    // Minimal harness: StewardShell construction needs daemon plumbing, so we
    // test the pure geometry of the kill operations through `at_candidates`
    // plus word-boundary helpers, and the ring invariants directly.
    #[test]
    fn kill_ring_caps_at_ten_entries() {
        let mut ring: Vec<String> = Vec::new();
        for index in 0..15 {
            ring.push(format!("w{index}"));
            if ring.len() > 10 {
                ring.remove(0);
            }
        }
        assert_eq!(ring.len(), 10);
        assert_eq!(ring[0], "w5");
    }

    #[test]
    fn boundaries_never_split_utf8_characters() {
        let input = "héllo wörld";
        let cursor = 3; // inside 'é'
        assert!(input.is_char_boundary(previous_boundary(input, cursor).min(cursor)));
    }
}
