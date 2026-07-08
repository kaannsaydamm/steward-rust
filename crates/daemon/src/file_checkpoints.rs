//! Automatic snapshots of files changed by `fs.write`, so any governed write can be undone.
//! Every write records the file's previous state (or its absence); `rollback` restores that
//! state and records the pre-rollback content as a fresh checkpoint, making rollback itself
//! reversible.

use anyhow::{bail, Context as _, Result};
use rusqlite::{params, Connection, OptionalExtension as _};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq)]
pub struct CheckpointInfo {
    pub checkpoint_id: i64,
    pub path: String,
    pub workspace_root: String,
    pub existed_before: bool,
    pub previous_bytes: usize,
    pub created_at: f64,
}

pub fn create_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS file_checkpoints (
            checkpoint_id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL,
            workspace_root TEXT NOT NULL,
            previous_content BLOB,
            created_at REAL NOT NULL
        );",
    )?;
    Ok(())
}

/// Records the state of `path` (relative to `workspace_root`) before a write.
/// `previous` is `None` when the file did not exist yet.
pub fn record(
    connection: &Connection,
    workspace_root: &str,
    path: &str,
    previous: Option<&[u8]>,
) -> Result<i64> {
    connection.execute(
        "INSERT INTO file_checkpoints (path, workspace_root, previous_content, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![path, workspace_root, previous, unix_seconds()],
    )?;
    Ok(connection.last_insert_rowid())
}

pub fn list(connection: &Connection, limit: usize) -> Result<Vec<CheckpointInfo>> {
    let mut statement = connection.prepare(
        "SELECT checkpoint_id, path, workspace_root,
                previous_content IS NOT NULL, COALESCE(LENGTH(previous_content), 0), created_at
         FROM file_checkpoints ORDER BY checkpoint_id DESC LIMIT ?1",
    )?;
    let checkpoints = statement
        .query_map([limit.clamp(1, 200)], |row| {
            Ok(CheckpointInfo {
                checkpoint_id: row.get(0)?,
                path: row.get(1)?,
                workspace_root: row.get(2)?,
                existed_before: row.get(3)?,
                previous_bytes: row.get::<_, i64>(4)?.max(0) as usize,
                created_at: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(checkpoints)
}

/// Restores the file to its checkpointed state: rewrites the previous content, or deletes the
/// file if it did not exist before the write. The content that was on disk immediately before
/// this rollback is recorded as a new checkpoint so the rollback can itself be undone.
pub fn rollback(connection: &Connection, checkpoint_id: i64) -> Result<String> {
    let row = connection
        .query_row(
            "SELECT path, workspace_root, previous_content
             FROM file_checkpoints WHERE checkpoint_id = ?1",
            [checkpoint_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<Vec<u8>>>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((path, workspace_root, previous)) = row else {
        bail!("checkpoint {checkpoint_id} not found");
    };
    let target = Path::new(&workspace_root).join(&path);
    let current = match std::fs::read(&target) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error).with_context(|| format!("reading {path} for rollback")),
    };
    record(connection, &workspace_root, &path, current.as_deref())?;
    match previous {
        Some(bytes) => {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating parent directories for {path}"))?;
            }
            std::fs::write(&target, &bytes).with_context(|| format!("restoring {path}"))?;
            Ok(format!("restored\t{path}\t{} bytes", bytes.len()))
        }
        None => {
            if target.exists() {
                std::fs::remove_file(&target).with_context(|| format!("removing {path}"))?;
            }
            Ok(format!("removed\t{path}\t(file did not exist before)"))
        }
    }
}

fn unix_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}

#[cfg(test)]
#[path = "file_checkpoints_tests.rs"]
mod tests;
