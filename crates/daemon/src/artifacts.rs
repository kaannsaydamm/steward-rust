use anyhow::{bail, Result};
use rusqlite::{params, Connection, OptionalExtension as _};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_TITLE_LEN: usize = 200;
const MAX_CONTENT_BYTES: usize = 2_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactKind {
    Code,
    Text,
    Markdown,
    Image,
    Link,
    Diff,
}

impl ArtifactKind {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Text => "text",
            Self::Markdown => "markdown",
            Self::Image => "image",
            Self::Link => "link",
            Self::Diff => "diff",
        }
    }

    pub fn parse(code: &str) -> Result<Self> {
        match code {
            "code" => Ok(Self::Code),
            "text" => Ok(Self::Text),
            "markdown" => Ok(Self::Markdown),
            "image" => Ok(Self::Image),
            "link" => Ok(Self::Link),
            "diff" => Ok(Self::Diff),
            other => bail!("unknown artifact kind '{other}'"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Artifact {
    pub artifact_id: String,
    pub title: String,
    pub kind: ArtifactKind,
    pub content: String,
    pub language: String,
    pub session_id: String,
    pub created_at: f64,
    pub updated_at: f64,
}

pub fn create_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS artifacts (
            artifact_id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            kind TEXT NOT NULL,
            content TEXT NOT NULL,
            language TEXT NOT NULL DEFAULT '',
            session_id TEXT NOT NULL DEFAULT '',
            created_at REAL NOT NULL,
            updated_at REAL NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_artifacts_created ON artifacts(created_at DESC);",
    )?;
    Ok(())
}

pub fn create(
    connection: &Connection,
    title: &str,
    kind: ArtifactKind,
    content: &str,
    language: &str,
    session_id: &str,
) -> Result<Artifact> {
    let title = title.trim();
    if title.is_empty() {
        bail!("artifact title is required");
    }
    if title.len() > MAX_TITLE_LEN {
        bail!("artifact title must be at most {MAX_TITLE_LEN} characters");
    }
    if content.len() > MAX_CONTENT_BYTES {
        bail!("artifact content exceeds the {MAX_CONTENT_BYTES}-byte limit");
    }
    let now = unix_seconds();
    let artifact = Artifact {
        artifact_id: uuid::Uuid::new_v4().to_string(),
        title: title.to_owned(),
        kind,
        content: content.to_owned(),
        language: language.to_owned(),
        session_id: session_id.to_owned(),
        created_at: now,
        updated_at: now,
    };
    connection.execute(
        "INSERT INTO artifacts
         (artifact_id, title, kind, content, language, session_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            artifact.artifact_id,
            artifact.title,
            artifact.kind.code(),
            artifact.content,
            artifact.language,
            artifact.session_id,
            artifact.created_at,
            artifact.updated_at,
        ],
    )?;
    Ok(artifact)
}

pub fn list(connection: &Connection, query: &str) -> Result<Vec<Artifact>> {
    let mut statement = connection.prepare(
        "SELECT artifact_id, title, kind, content, language, session_id, created_at, updated_at
         FROM artifacts
         WHERE ?1 = '' OR title LIKE '%' || ?1 || '%' OR content LIKE '%' || ?1 || '%'
         ORDER BY created_at DESC",
    )?;
    let rows = statement.query_map([query], row_to_artifact)?;
    let mut artifacts = Vec::new();
    for row in rows {
        artifacts.push(row??);
    }
    Ok(artifacts)
}

pub fn get(connection: &Connection, artifact_id: &str) -> Result<Option<Artifact>> {
    let row = connection
        .query_row(
            "SELECT artifact_id, title, kind, content, language, session_id, created_at, updated_at
             FROM artifacts WHERE artifact_id = ?1",
            [artifact_id],
            row_to_artifact,
        )
        .optional()?;
    row.transpose()
}

pub fn delete(connection: &Connection, artifact_id: &str) -> Result<bool> {
    let deleted = connection.execute(
        "DELETE FROM artifacts WHERE artifact_id = ?1",
        [artifact_id],
    )?;
    Ok(deleted > 0)
}

fn row_to_artifact(row: &rusqlite::Row) -> rusqlite::Result<Result<Artifact>> {
    let kind: String = row.get(2)?;
    Ok(ArtifactKind::parse(&kind).map(|kind| Artifact {
        artifact_id: row.get::<_, String>(0).unwrap_or_default(),
        title: row.get::<_, String>(1).unwrap_or_default(),
        kind,
        content: row.get::<_, String>(3).unwrap_or_default(),
        language: row.get::<_, String>(4).unwrap_or_default(),
        session_id: row.get::<_, String>(5).unwrap_or_default(),
        created_at: row.get::<_, f64>(6).unwrap_or_default(),
        updated_at: row.get::<_, f64>(7).unwrap_or_default(),
    }))
}

fn unix_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}

#[cfg(test)]
#[path = "artifacts_tests.rs"]
mod tests;
