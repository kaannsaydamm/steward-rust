use super::{HistoryLine, LineKind, ShellState};
use steward_core::pb::{ChatEvent, ChatEventKind};

#[test]
fn chat_events_update_session_transcript_and_usage() {
    let mut state = ShellState::new("http://127.0.0.1:50051".to_owned());
    state.busy = true;
    state.apply_chat_event(ChatEvent {
        session_id: "session-1".to_owned(),
        kind: ChatEventKind::Session as i32,
        content: "session ready".to_owned(),
        ..ChatEvent::default()
    });
    state.apply_chat_event(ChatEvent {
        session_id: "session-1".to_owned(),
        kind: ChatEventKind::Text as i32,
        content: "answer".to_owned(),
        ..ChatEvent::default()
    });
    state.apply_chat_event(ChatEvent {
        session_id: "session-1".to_owned(),
        kind: ChatEventKind::Done as i32,
        prompt_tokens: 12,
        completion_tokens: 7,
        ..ChatEvent::default()
    });

    assert_eq!(state.session_id.as_deref(), Some("session-1"));
    assert_eq!(state.prompt_tokens, 12);
    assert_eq!(state.completion_tokens, 7);
    assert!(!state.busy);
    assert!(
        matches!(state.history.last(), Some(HistoryLine { kind: LineKind::Agent, text }) if text == "answer")
    );
}

#[test]
fn tool_events_are_visible_in_activity_and_transcript() {
    let mut state = ShellState::new("http://127.0.0.1:50051".to_owned());

    state.apply_chat_event(ChatEvent {
        kind: ChatEventKind::ToolStart as i32,
        tool_name: "memory.recall".to_owned(),
        arguments_json: "{\"query\":\"rust\"}".to_owned(),
        ..ChatEvent::default()
    });

    assert_eq!(state.tool_activity.len(), 1);
    assert!(matches!(
        state.history.last(),
        Some(HistoryLine {
            kind: LineKind::Tool,
            ..
        })
    ));
}
