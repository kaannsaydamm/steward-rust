//! Versioned, reversible-aware database migrations (Omega Task 1.2 / §48).
//!
//! Contract:
//! - schema version lives in `schema_migrations`;
//! - the first migration backs up the pre-migration database under
//!   `<data_root>/backups/`;
//! - every migration runs in one transaction;
//! - old source tables are never dropped by the migration that replaces them.

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

/// One ordered schema migration.
pub trait Migration {
    fn version(&self) -> i64;
    fn name(&self) -> &'static str;
    fn up(&self, tx: &rusqlite::Transaction<'_>) -> Result<()>;
}

/// Registry of all Omega migrations in version order.
pub fn all_migrations() -> Vec<Box<dyn Migration + Send + Sync>> {
    vec![
        Box::new(v0001_baseline::Baseline),
        Box::new(v0002_threads_runs::ThreadsRuns),
    ]
}

/// Current schema version recorded in the database (0 = never migrated).
pub fn current_version(connection: &Connection) -> Result<i64> {
    ensure_migrations_table(connection)?;
    let version: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .context("reading schema version")?;
    Ok(version)
}

fn ensure_migrations_table(connection: &Connection) -> Result<()> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at_ms INTEGER NOT NULL
            )",
        )
        .context("creating schema_migrations table")
}

/// Applies all pending migrations. `data_root` receives the pre-migration
/// backup when at least one migration runs.
pub fn migrate(connection: &mut Connection, data_root: &Path) -> Result<Vec<i64>> {
    ensure_migrations_table(connection)?;
    let migrations = all_migrations();
    let current = current_version(connection)?;

    let pending: Vec<&(dyn Migration + Send + Sync)> = migrations
        .iter()
        .map(|m| m.as_ref())
        .filter(|m| m.version() > current)
        .collect();

    if pending.is_empty() {
        return Ok(Vec::new());
    }

    let backup = backup_database(connection, data_root)
        .context("backing up database before migration")?;
    tracing::info!(?backup, "pre-migration backup created");

    let mut applied = Vec::with_capacity(pending.len());
    for migration in pending {
        let version = migration.version();
        let name = migration.name();
        let tx = connection.transaction().context("opening migration tx")?;
        migration.up(&tx).with_context(|| format!("migration {version} ({name})"))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, applied_at_ms) VALUES (?1, ?2, ?3)",
            rusqlite::params![
                version,
                name,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64
            ],
        )
        .context("recording migration")?;
        tx.commit()
            .with_context(|| format!("committing migration {version}"))?;
        applied.push(version);
    }
    Ok(applied)
}

/// Timestamped SQLite backup via the online backup API.
pub fn backup_database(connection: &Connection, data_root: &Path) -> Result<PathBuf> {
    let directory = data_root.join("backups");
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("creating backup dir {}", directory.display()))?;
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S");
    let target = directory.join(format!("steward-{timestamp}.db"));
    let mut target_connection = Connection::open(&target)
        .with_context(|| format!("opening backup target {}", target.display()))?;
    let backup = rusqlite::backup::Backup::new(connection, &mut target_connection)
        .context("starting backup")?;
    backup
        .run_to_completion(64, std::time::Duration::from_millis(5), None)
        .context("running backup")?;
    Ok(target)
}

mod v0001_baseline {
    use super::{Migration, Result};
    use rusqlite::Transaction;

    /// v1 databases predate the migrator; this version records their schema
    /// as the starting point without touching any object.
    pub(super) struct Baseline;

    impl Migration for Baseline {
        fn version(&self) -> i64 {
            1
        }
        fn name(&self) -> &'static str {
            "baseline"
        }
        fn up(&self, _tx: &Transaction<'_>) -> Result<()> {
            Ok(())
        }
    }
}

mod v0002_threads_runs {
    use super::{Migration, Result};
    use anyhow::Context as _;
    use rusqlite::Transaction;
    /// Omega §49 durable run substrate: threads + runs + ordered events +
    /// checkpoints + node attempts + agent instances + scoped approvals.
    pub(super) struct ThreadsRuns;

    impl Migration for ThreadsRuns {
        fn version(&self) -> i64 {
            2
        }
        fn name(&self) -> &'static str {
            "threads_runs_events_checkpoints"
        }
        fn up(&self, tx: &Transaction<'_>) -> Result<()> {
            tx.execute_batch(include_str!("migrations/v0002_threads_runs.sql"))
                .context("executing v0002 schema")?;
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "db_migrator_tests.rs"]
mod tests;
