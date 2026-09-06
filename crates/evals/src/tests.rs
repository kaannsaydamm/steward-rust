use super::fake_model::{call, RecordedMessage, RecordedTool, ScriptedModel};
use serde_json::json;

fn user_message(content: &str) -> RecordedMessage {
    RecordedMessage {
        role: "user",
        content: content.to_owned(),
        tool_call_id: None,
    }
}

fn tool_result_message(id: &str, content: &str) -> RecordedMessage {
    RecordedMessage {
        role: "tool",
        content: content.to_owned(),
        tool_call_id: Some(id.to_owned()),
    }
}

fn read_tool() -> RecordedTool {
    RecordedTool {
        name: "fs.read".to_owned(),
        description: "read a file".to_owned(),
        input_schema: json!({"type": "object"}),
    }
}

#[test]
fn scripts_text_only_completion() {
    let model = ScriptedModel::new().text("hello");
    let reply = model
        .complete(&[user_message("hi")], &[])
        .expect("scripted reply");
    assert_eq!(reply.text, "hello");
    assert!(reply.tool_calls.is_empty());
    assert_eq!(reply.prompt_tokens, 10);
    assert_eq!(reply.completion_tokens, 5);
    assert_eq!(model.remaining(), 0);
}

#[test]
fn scripts_text_to_tool_call_to_text_and_records_exact_history() {
    let model = ScriptedModel::new()
        .tool(vec![call("fs.read", json!({"path": "README.md"}))])
        .text("done");

    let tools = vec![read_tool()];
    let first = model
        .complete(&[user_message("read the readme")], &tools)
        .expect("first reply");
    assert_eq!(first.tool_calls.len(), 1);
    assert_eq!(first.tool_calls[0].id, "call_fs.read");
    assert_eq!(first.tool_calls[0].name, "fs.read");
    assert_eq!(first.tool_calls[0].arguments, json!({"path": "README.md"}));

    let second = model
        .complete(
            &[
                user_message("read the readme"),
                tool_result_message("call_fs.read", "file contents"),
            ],
            &tools,
        )
        .expect("second reply");
    assert_eq!(second.text, "done");

    let requests = model.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].messages, vec![user_message("read the readme")]);
    assert_eq!(requests[0].tools, tools);
    assert_eq!(
        requests[1].messages,
        vec![
            user_message("read the readme"),
            tool_result_message("call_fs.read", "file contents"),
        ]
    );
    // Tool definitions are injected on every call exactly as the caller sent them.
    assert_eq!(requests[1].tools, tools);
}

#[test]
fn error_replies_surface_deterministically() {
    let model = ScriptedModel::new().error("provider down");
    let error = model
        .complete(&[user_message("hi")], &[])
        .expect_err("scripted error");
    assert!(error.to_string().contains("provider down"));
}

#[test]
#[should_panic(expected = "scripted model exhausted")]
fn exhausted_queue_is_an_authoring_bug() {
    let model = ScriptedModel::new();
    let _ = model.complete(&[user_message("hi")], &[]).unwrap();
}

#[test]
fn tool_call_serialization_is_stable() {
    let a = call("fs.edit", json!({"path": "a.rs", "b": [1, 2]}));
    let b = call("fs.edit", json!({"b": [1, 2], "path": "a.rs"}));
    // Argument objects serialize identically regardless of construction order.
    assert_eq!(
        serde_json::to_string(&a.arguments).unwrap(),
        serde_json::to_string(&b.arguments).unwrap()
    );
}

#[tokio::test]
async fn scripted_model_is_shareable_across_tasks() {
    let model = ScriptedModel::new().text("one").text("two");
    let m1 = model.clone();
    let t1 = tokio::spawn(async move { m1.complete(&[user_message("a")], &[]).unwrap().text });
    let m2 = model.clone();
    let t2 = tokio::spawn(async move { m2.complete(&[user_message("b")], &[]).unwrap().text });
    let (r1, r2) = tokio::join!(t1, t2);
    assert_eq!(r1.unwrap(), "one");
    assert_eq!(r2.unwrap(), "two");
}
