use anyhow::{bail, Result};
use rusqlite::{params, Connection, OptionalExtension as _};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq)]
pub struct SessionSummary {
    pub session_id: String,
    pub title: String,
    pub provider_profile: String,
    pub model: String,
    pub created_at: f64,
    pub updated_at: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredMessage {
    pub message_id: i64,
    pub role: String,
    pub content: String,
    pub tool_name: String,
    pub tool_call_id: String,
    pub created_at: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredSession {
    pub summary: SessionSummary,
    pub messages: Vec<StoredMessage>,
}

pub fn create_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS chat_sessions (
            session_id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            provider_profile TEXT NOT NULL,
            model TEXT NOT NULL,
            created_at REAL NOT NULL,
            updated_at REAL NOT NULL
        );
        CREATE TABLE IF NOT EXISTS chat_messages (
            message_id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL REFERENCES chat_sessions(session_id) ON DELETE CASCADE,
            role TEXT NOT NULL CHECK(role IN ('system','user','assistant','tool')),
            content TEXT NOT NULL,
            tool_name TEXT NOT NULL DEFAULT '',
            tool_call_id TEXT NOT NULL DEFAULT '',
            created_at REAL NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_chat_messages_session
            ON chat_messages(session_id, message_id);",
    )?;
    Ok(())
}

pub fn create_session(
    connection: &Connection,
    provider_profile: &str,
    model: &str,
    title: &str,
) -> Result<SessionSummary> {
    if provider_profile.trim().is_empty() || model.trim().is_empty() {
        bail!("provider profile and model are required");
    }
    let session = SessionSummary {
        session_id: uuid::Uuid::new_v4().to_string(),
        title: truncate_title(title),
        provider_profile: provider_profile.to_owned(),
        model: model.to_owned(),
        created_at: unix_seconds(),
        updated_at: unix_seconds(),
    };
    connection.execute(
        "INSERT INTO chat_sessions
         (session_id, title, provider_profile, model, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            session.session_id,
            session.title,
            session.provider_profile,
            session.model,
            session.created_at,
            session.updated_at
        ],
    )?;
    Ok(session)
}

pub fn append_message(
    connection: &Connection,
    session_id: &str,
    role: &str,
    content: &str,
    tool_name: &str,
    tool_call_id: &str,
) -> Result<i64> {
    if !matches!(role, "system" | "user" | "assistant" | "tool") {
        bail!("unsupported session message role '{role}'");
    }
    let now = unix_seconds();
    connection.execute(
        "INSERT INTO chat_messages
         (session_id, role, content, tool_name, tool_call_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![session_id, role, content, tool_name, tool_call_id, now],
    )?;
    connection.execute(
        "UPDATE chat_sessions SET updated_at = ?2 WHERE session_id = ?1",
        params![session_id, now],
    )?;
    Ok(connection.last_insert_rowid())
}

pub fn get_session(connection: &Connection, session_id: &str) -> Result<Option<StoredSession>> {
    get_session_since(connection, session_id, 0)
}

/// Reads a session, optionally resuming from a sequence point: messages with
/// `message_id > since_sequence` are returned (0 = full replay). This is the
/// Steward equivalent of Hermes gateway `session.events.since` resume.
pub fn get_session_since(
    connection: &Connection,
    session_id: &str,
    since_sequence: i64,
) -> Result<Option<StoredSession>> {
    let summary = connection
        .query_row(
            "SELECT session_id, title, provider_profile, model, created_at, updated_at
             FROM chat_sessions WHERE session_id = ?1",
            [session_id],
            summary_from_row,
        )
        .optional()?;
    let Some(summary) = summary else {
        return Ok(None);
    };
    let mut statement = connection.prepare(
        "SELECT message_id, role, content, tool_name, tool_call_id, created_at
         FROM chat_messages WHERE session_id = ?1 AND message_id > ?2 ORDER BY message_id",
    )?;
    let messages = statement
        .query_map(params![session_id, since_sequence], |row| {
            Ok(StoredMessage {
                message_id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                tool_name: row.get(3)?,
                tool_call_id: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Some(StoredSession { summary, messages }))
}

pub fn list_sessions(connection: &Connection, limit: usize) -> Result<Vec<SessionSummary>> {
    let mut statement = connection.prepare(
        "SELECT session_id, title, provider_profile, model, created_at, updated_at
         FROM chat_sessions ORDER BY updated_at DESC, rowid DESC LIMIT ?1",
    )?;
    let sessions = statement
        .query_map([limit.clamp(1, 200)], summary_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(sessions)
}

/// Replaces every stored message with one system-role summary, returning how many messages
/// were removed. This is the storage half of session compaction — the caller is responsible
/// for producing the summary text (normally by asking the active model).
pub fn replace_with_summary(
    connection: &Connection,
    session_id: &str,
    summary: &str,
) -> Result<usize> {
    if summary.trim().is_empty() {
        bail!("compaction summary cannot be empty");
    }
    let transaction = connection.unchecked_transaction()?;
    let removed = transaction.execute(
        "DELETE FROM chat_messages WHERE session_id = ?1",
        [session_id],
    )?;
    if removed == 0 {
        bail!("session '{session_id}' has no messages to compact");
    }
    transaction.execute(
        "INSERT INTO chat_messages (session_id, role, content, tool_name, tool_call_id, created_at)
         VALUES (?1, 'system', ?2, '', '', ?3)",
        params![
            session_id,
            format!("Summary of the conversation so far:\n{summary}"),
            unix_seconds()
        ],
    )?;
    transaction.commit()?;
    Ok(removed)
}

pub fn delete_session(connection: &Connection, session_id: &str) -> Result<bool> {
    Ok(connection.execute(
        "DELETE FROM chat_sessions WHERE session_id = ?1",
        [session_id],
    )? > 0)
}

fn summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionSummary> {
    Ok(SessionSummary {
        session_id: row.get(0)?,
        title: row.get(1)?,
        provider_profile: row.get(2)?,
        model: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn truncate_title(value: &str) -> String {
    let title = value.trim().lines().next().unwrap_or("New session");
    title.chars().take(80).collect()
}

fn unix_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}

#[cfg(test)]
#[path = "session_store_tests.rs"]
mod tests;
