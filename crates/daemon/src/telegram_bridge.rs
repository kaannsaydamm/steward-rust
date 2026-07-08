//! "Claude Dispatch"-style channel bridge: connect a Telegram bot to the local agent runtime
//! so the operator can talk to Steward from their phone. Long-polls getUpdates and routes each
//! allowed chat's messages through the normal governed chat path (same session store, same
//! tools, same approvals) — one persistent Steward session per Telegram chat.
//!
//! Access is default-deny: chats not on the allowlist only ever receive pairing instructions
//! containing their chat id, never model output. Configure with
//! `steward channel set-telegram <token>` and `steward channel allow <chat_id>`.

use crate::{agent_runtime, MySteward};
use anyhow::{Context as _, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use steward_core::channel_settings::ChannelSettings;
use steward_core::pb::ChatRequest;

const POLL_TIMEOUT_SECONDS: u64 = 50;
const ERROR_BACKOFF: Duration = Duration::from_secs(5);

pub fn spawn(steward: MySteward, settings_path: PathBuf) {
    let settings = match ChannelSettings::load(&settings_path) {
        Ok(settings) => settings,
        Err(error) => {
            tracing::warn!("channel settings unreadable, Telegram bridge disabled: {error:#}");
            return;
        }
    };
    let Some(token) = settings.telegram_token.clone().filter(|t| !t.is_empty()) else {
        return;
    };
    tracing::info!("Telegram bridge starting (long polling)");
    tokio::spawn(async move {
        run_loop(steward, settings_path, token).await;
    });
}

async fn run_loop(steward: MySteward, settings_path: PathBuf, token: String) {
    let mut offset: i64 = 0;
    // One Steward chat session per Telegram chat, so context persists across messages.
    let mut sessions: HashMap<i64, String> = HashMap::new();
    loop {
        match fetch_updates(&steward, &token, offset).await {
            Ok(updates) => {
                for update in updates {
                    let Some(update_id) = update.get("update_id").and_then(Value::as_i64) else {
                        continue;
                    };
                    offset = offset.max(update_id + 1);
                    let Some((chat_id, text)) = extract_message(&update) else {
                        continue;
                    };
                    // Re-read the allowlist on every message so `steward channel allow` takes
                    // effect without restarting the daemon.
                    let allowed = ChannelSettings::load(&settings_path)
                        .map(|settings| settings.is_chat_allowed(chat_id))
                        .unwrap_or(false);
                    if !allowed {
                        let pairing = format!(
                            "This Steward bridge does not know this chat yet.\n\
                             To pair it, run on the host machine:\n\
                             steward channel allow {chat_id}"
                        );
                        if let Err(error) = send_message(&steward, &token, chat_id, &pairing).await
                        {
                            tracing::warn!("telegram pairing reply failed: {error:#}");
                        }
                        continue;
                    }
                    let session_id = sessions.get(&chat_id).cloned().unwrap_or_default();
                    match chat_turn(&steward, &session_id, &text).await {
                        Ok((new_session, reply)) => {
                            sessions.insert(chat_id, new_session);
                            let reply = if reply.trim().is_empty() {
                                "(no response)".to_owned()
                            } else {
                                reply
                            };
                            if let Err(error) =
                                send_message(&steward, &token, chat_id, &reply).await
                            {
                                tracing::warn!("telegram reply failed: {error:#}");
                            }
                        }
                        Err(error) => {
                            let message = format!("Steward error: {error:#}");
                            let _ = send_message(&steward, &token, chat_id, &message).await;
                        }
                    }
                }
            }
            Err(error) => {
                tracing::warn!("telegram getUpdates failed: {error:#}");
                tokio::time::sleep(ERROR_BACKOFF).await;
            }
        }
    }
}

async fn fetch_updates(steward: &MySteward, token: &str, offset: i64) -> Result<Vec<Value>> {
    let url = format!(
        "https://api.telegram.org/bot{token}/getUpdates?timeout={POLL_TIMEOUT_SECONDS}&offset={offset}&allowed_updates=%5B%22message%22%5D"
    );
    let response = steward
        .http
        .get(&url)
        .timeout(Duration::from_secs(POLL_TIMEOUT_SECONDS + 10))
        .send()
        .await
        .context("calling Telegram getUpdates")?;
    let body: Value = response
        .json()
        .await
        .context("parsing Telegram getUpdates response")?;
    if !body.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        anyhow::bail!(
            "Telegram API error: {}",
            body.get("description")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        );
    }
    Ok(body
        .get("result")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

fn extract_message(update: &Value) -> Option<(i64, String)> {
    let message = update.get("message")?;
    let chat_id = message.get("chat")?.get("id")?.as_i64()?;
    let text = message.get("text")?.as_str()?.to_owned();
    (!text.trim().is_empty()).then_some((chat_id, text))
}

async fn chat_turn(
    steward: &MySteward,
    session_id: &str,
    message: &str,
) -> Result<(String, String)> {
    let (sender, mut receiver) = tokio::sync::mpsc::channel(64);
    // Drain events so the bounded channel never blocks the runtime; the bridge only needs
    // the final text, which `run` returns directly.
    let drain = tokio::spawn(async move { while receiver.recv().await.is_some() {} });
    let result = agent_runtime::run(
        steward,
        ChatRequest {
            session_id: session_id.to_owned(),
            message: message.to_owned(),
            working_directory: String::new(),
            allow_tools: true,
        },
        sender,
    )
    .await;
    drain.abort();
    let outcome = result?;
    Ok((outcome.session_id, outcome.final_text))
}

async fn send_message(steward: &MySteward, token: &str, chat_id: i64, text: &str) -> Result<()> {
    // Telegram caps messages at 4096 chars; split long replies.
    for chunk in split_chunks(text, 4000) {
        let url = format!("https://api.telegram.org/bot{token}/sendMessage");
        let response = steward
            .http
            .post(&url)
            .json(&serde_json::json!({ "chat_id": chat_id, "text": chunk }))
            .send()
            .await
            .context("calling Telegram sendMessage")?;
        if !response.status().is_success() {
            anyhow::bail!("Telegram sendMessage returned {}", response.status());
        }
    }
    Ok(())
}

fn split_chunks(text: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for line in text.split_inclusive('\n') {
        if current.chars().count() + line.chars().count() > max_chars && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
        }
        // A single line longer than the cap is hard-split.
        if line.chars().count() > max_chars {
            let characters: Vec<char> = line.chars().collect();
            for piece in characters.chunks(max_chars) {
                chunks.push(piece.iter().collect());
            }
        } else {
            current.push_str(line);
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    if chunks.is_empty() {
        chunks.push(String::new());
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::{extract_message, split_chunks};
    use serde_json::json;

    #[test]
    fn extract_message_reads_chat_id_and_text() {
        let update = json!({
            "update_id": 1,
            "message": { "chat": { "id": 42 }, "text": "hello" }
        });
        assert_eq!(extract_message(&update), Some((42, "hello".to_owned())));
    }

    #[test]
    fn extract_message_ignores_non_text_updates() {
        let update = json!({ "update_id": 1, "message": { "chat": { "id": 42 } } });
        assert_eq!(extract_message(&update), None);
    }

    #[test]
    fn split_chunks_keeps_short_text_whole() {
        assert_eq!(split_chunks("hello", 100), vec!["hello".to_owned()]);
    }

    #[test]
    fn split_chunks_splits_on_line_boundaries() {
        let text = "aaaa\nbbbb\ncccc\n";
        let chunks = split_chunks(text, 10);
        assert!(chunks.len() >= 2);
        assert_eq!(chunks.concat(), text);
    }
}
