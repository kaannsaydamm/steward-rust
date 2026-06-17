use super::StewardShell;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

#[test]
fn unicode_prompt_editing_uses_character_boundaries() {
    let mut shell = StewardShell::new("http://127.0.0.1:7777".to_owned());

    shell.insert_char('ğ');
    shell.insert_char('x');
    shell.move_cursor_left();
    shell.backspace();

    assert_eq!(shell.state.input, "x");
    assert_eq!(shell.state.input_cursor, 0);
}

#[test]
fn command_history_preserves_draft_and_skips_duplicates() {
    let mut shell = StewardShell::new("http://127.0.0.1:7777".to_owned());
    shell.push_command_history("/status".to_owned());
    shell.push_command_history("/status".to_owned());
    shell.state.input = "/wor".to_owned();
    shell.state.input_cursor = shell.state.input.len();

    shell.history_prev();
    assert_eq!(shell.state.input, "/status");
    assert_eq!(shell.command_history.len(), 1);

    shell.history_next();
    assert_eq!(shell.state.input, "/wor");
}

#[test]
fn clear_history_keeps_operator_feedback() {
    let mut shell = StewardShell::new("http://127.0.0.1:7777".to_owned());
    shell.state.push_system("old line");

    shell.clear_history();

    assert_eq!(shell.state.history.len(), 1);
    assert_eq!(shell.state.history[0].text, "transcript cleared");
}

#[tokio::test]
async fn key_release_events_do_not_duplicate_input() {
    let mut shell = StewardShell::new("http://127.0.0.1:7777".to_owned());
    let key = KeyEvent {
        code: KeyCode::Char('/'),
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Release,
        state: KeyEventState::empty(),
    };

    let should_exit = shell.handle_key(key).await.expect("release key handling");

    assert!(!should_exit);
    assert!(shell.state.input.is_empty());
}
