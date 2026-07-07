use crate::MySteward;
use anyhow::{bail, Result};
use log::{info, warn};
use rusqlite::{params, Connection, OptionalExtension as _};
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MIN_INTERVAL_SECONDS: i64 = 60;
const MAX_INTERVAL_SECONDS: i64 = 30 * 24 * 60 * 60;
const POLL_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, PartialEq)]
pub struct CronJob {
    pub job_id: String,
    pub name: String,
    pub tool_id: String,
    pub input_json: String,
    pub interval_seconds: i64,
    pub enabled: bool,
    pub last_run_at: f64,
    pub last_status: String,
    pub created_at: f64,
}

pub fn create_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS cron_jobs (
            job_id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            tool_id TEXT NOT NULL,
            input_json TEXT NOT NULL DEFAULT '{}',
            interval_seconds INTEGER NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 1,
            last_run_at REAL NOT NULL DEFAULT 0,
            last_status TEXT NOT NULL DEFAULT '',
            created_at REAL NOT NULL
        );",
    )?;
    Ok(())
}

pub fn create_job(
    connection: &Connection,
    name: &str,
    tool_id: &str,
    input_json: &str,
    interval_seconds: i64,
) -> Result<CronJob> {
    if name.trim().is_empty() {
        bail!("cron job name is required");
    }
    if tool_id.trim().is_empty() {
        bail!("tool_id is required");
    }
    if !(MIN_INTERVAL_SECONDS..=MAX_INTERVAL_SECONDS).contains(&interval_seconds) {
        bail!("interval_seconds must be between {MIN_INTERVAL_SECONDS} and {MAX_INTERVAL_SECONDS}");
    }
    if !input_json.trim().is_empty() {
        serde_json::from_str::<serde_json::Value>(input_json)
            .map_err(|error| anyhow::anyhow!("input_json is not valid JSON: {error}"))?;
    }
    let job = CronJob {
        job_id: uuid::Uuid::new_v4().to_string(),
        name: name.trim().to_owned(),
        tool_id: tool_id.trim().to_owned(),
        input_json: if input_json.trim().is_empty() {
            "{}".to_owned()
        } else {
            input_json.to_owned()
        },
        interval_seconds,
        enabled: true,
        last_run_at: 0.0,
        last_status: String::new(),
        created_at: unix_seconds(),
    };
    connection.execute(
        "INSERT INTO cron_jobs
         (job_id, name, tool_id, input_json, interval_seconds, enabled, last_run_at, last_status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            job.job_id,
            job.name,
            job.tool_id,
            job.input_json,
            job.interval_seconds,
            job.enabled,
            job.last_run_at,
            job.last_status,
            job.created_at,
        ],
    )?;
    Ok(job)
}

pub fn list_jobs(connection: &Connection) -> Result<Vec<CronJob>> {
    let mut statement = connection.prepare(
        "SELECT job_id, name, tool_id, input_json, interval_seconds, enabled, last_run_at, last_status, created_at
         FROM cron_jobs ORDER BY created_at DESC",
    )?;
    let rows = statement.query_map([], row_to_job)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn get_job(connection: &Connection, job_id: &str) -> Result<Option<CronJob>> {
    connection
        .query_row(
            "SELECT job_id, name, tool_id, input_json, interval_seconds, enabled, last_run_at, last_status, created_at
             FROM cron_jobs WHERE job_id = ?1",
            [job_id],
            row_to_job,
        )
        .optional()
        .map_err(Into::into)
}

pub fn set_enabled(connection: &Connection, job_id: &str, enabled: bool) -> Result<CronJob> {
    let updated = connection.execute(
        "UPDATE cron_jobs SET enabled = ?1 WHERE job_id = ?2",
        params![enabled, job_id],
    )?;
    if updated == 0 {
        bail!("cron job '{job_id}' does not exist");
    }
    get_job(connection, job_id)?
        .ok_or_else(|| anyhow::anyhow!("cron job '{job_id}' disappeared after update"))
}

pub fn delete_job(connection: &Connection, job_id: &str) -> Result<bool> {
    let deleted = connection.execute("DELETE FROM cron_jobs WHERE job_id = ?1", [job_id])?;
    Ok(deleted > 0)
}

pub fn mark_run(connection: &Connection, job_id: &str, status: &str) -> Result<()> {
    connection.execute(
        "UPDATE cron_jobs SET last_run_at = ?1, last_status = ?2 WHERE job_id = ?3",
        params![unix_seconds(), status, job_id],
    )?;
    Ok(())
}

/// Jobs whose interval has elapsed since their last run. `enabled` jobs only.
pub fn due_jobs(connection: &Connection, now: f64) -> Result<Vec<CronJob>> {
    Ok(list_jobs(connection)?
        .into_iter()
        .filter(|job| job.enabled && now - job.last_run_at >= job.interval_seconds as f64)
        .collect())
}

fn row_to_job(row: &rusqlite::Row) -> rusqlite::Result<CronJob> {
    Ok(CronJob {
        job_id: row.get(0)?,
        name: row.get(1)?,
        tool_id: row.get(2)?,
        input_json: row.get(3)?,
        interval_seconds: row.get(4)?,
        enabled: row.get(5)?,
        last_run_at: row.get(6)?,
        last_status: row.get(7)?,
        created_at: row.get(8)?,
    })
}

/// Runs cron-scheduled tool invocations are considered pre-approved by the
/// user who created the job (choosing a tool_id + arguments to automate is
/// itself the approval, mirroring `steward tool invoke --approve`).
const CRON_APPROVED: bool = true;

pub fn spawn_scheduler(steward: MySteward) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;
            if let Err(error) = run_due_jobs(&steward).await {
                warn!("cron scheduler tick failed: {error:#}");
            }
        }
    });
}

async fn run_due_jobs(steward: &MySteward) -> Result<()> {
    let now = unix_seconds();
    let due = {
        let connection = steward
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
        due_jobs(&connection, now)?
    };
    for job in due {
        let working_directory = steward_core::storage::root()
            .map(|root| root.to_string_lossy().into_owned())
            .unwrap_or_default();
        let arguments = json_object_to_arguments(&job.input_json);
        let outcome = crate::tool_invocation::invoke(
            steward,
            &job.tool_id,
            arguments,
            CRON_APPROVED,
            &working_directory,
        )
        .await;
        let status = match &outcome {
            Ok(Some(result)) => format!("{:?}", result.status),
            Ok(None) => "tool not found".to_owned(),
            Err(error) => format!("error: {error:#}"),
        };
        info!("cron job '{}' ({}) ran: {status}", job.name, job.job_id);
        let connection = steward
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
        mark_run(&connection, &job.job_id, &status)?;
    }
    Ok(())
}

fn json_object_to_arguments(input_json: &str) -> BTreeMap<String, String> {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str(input_json) else {
        return BTreeMap::new();
    };
    map.into_iter()
        .map(|(key, value)| {
            let value = match value {
                serde_json::Value::String(text) => text,
                other => other.to_string(),
            };
            (key, value)
        })
        .collect()
}

fn unix_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}

#[cfg(test)]
#[path = "cron_jobs_tests.rs"]
mod tests;
