use anyhow::{bail, Context as _, Result};
use rusqlite::Connection;
use serde::Deserialize;
use std::path::Path;

const SECONDS_PER_DAY: f64 = 86_400.0;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct RetentionConfig {
    pub retention_days: u16,
    pub max_completed_workflows: u16,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            retention_days: 30,
            max_completed_workflows: 200,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PruneReport {
    pub tool_invocations: usize,
    pub workflows: usize,
}

pub fn load_config(root: &Path) -> Result<RetentionConfig> {
    let path = root.join("config.json");
    if !path.exists() {
        return Ok(RetentionConfig::default());
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    parse_config(&text).with_context(|| format!("parsing {}", path.display()))
}

fn parse_config(text: &str) -> Result<RetentionConfig> {
    let config: RetentionConfig = serde_json::from_str(text)?;
    if config.retention_days == 0 {
        bail!("retention_days must be between 1 and 3650");
    }
    if config.retention_days > 3650 {
        bail!("retention_days must be between 1 and 3650");
    }
    if config.max_completed_workflows == 0 || config.max_completed_workflows > 10_000 {
        bail!("max_completed_workflows must be between 1 and 10000");
    }
    Ok(config)
}

pub fn prune(db: &Connection, config: RetentionConfig, now: f64) -> Result<PruneReport> {
    let cutoff = now - f64::from(config.retention_days) * SECONDS_PER_DAY;
    let transaction = db.unchecked_transaction()?;
    let tool_invocations = transaction.execute(
        "DELETE FROM tool_invocations WHERE created_at < ?1",
        [cutoff],
    )?;
    transaction.execute(
        "CREATE TEMP TABLE IF NOT EXISTS steward_prune_workflows (workflow_id TEXT PRIMARY KEY)",
        [],
    )?;
    transaction.execute("DELETE FROM steward_prune_workflows", [])?;
    transaction.execute(
        "INSERT INTO steward_prune_workflows
         SELECT workflow_id FROM workflow_runs
         WHERE (phase >= 10 OR cancelled = 1) AND updated_at < ?1",
        [cutoff],
    )?;
    transaction.execute(
        "INSERT OR IGNORE INTO steward_prune_workflows
         SELECT workflow_id FROM workflow_runs
         WHERE phase >= 10 OR cancelled = 1
         ORDER BY updated_at DESC
         LIMIT -1 OFFSET ?1",
        [config.max_completed_workflows],
    )?;
    transaction.execute(
        "DELETE FROM workflow_events WHERE workflow_id IN (SELECT workflow_id FROM steward_prune_workflows)",
        [],
    )?;
    let workflows = transaction.execute(
        "DELETE FROM workflow_runs WHERE workflow_id IN (SELECT workflow_id FROM steward_prune_workflows)",
        [],
    )?;
    transaction.commit()?;
    Ok(PruneReport {
        tool_invocations,
        workflows,
    })
}

pub fn is_loopback_origin(origin: &str) -> bool {
    let Some(authority) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    else {
        return false;
    };
    let host = authority.split('/').next().unwrap_or(authority);
    if host == "[::1]" || host.starts_with("[::1]:") {
        return true;
    }
    let host = host.split(':').next().unwrap_or(host);
    matches!(host, "localhost" | "127.0.0.1")
}

#[cfg(test)]
mod tests {
    use super::{is_loopback_origin, parse_config, prune, RetentionConfig};
    use crate::{tool_registry, workflow_store};
    use rusqlite::Connection;

    #[test]
    fn config_rejects_zero_retention_days() {
        let error = parse_config(r#"{"retention_days":0}"#).expect_err("invalid config");
        assert!(error.to_string().contains("retention_days"));
    }

    #[test]
    fn origin_policy_allows_only_loopback_hosts() {
        assert!(is_loopback_origin("http://localhost:3000"));
        assert!(is_loopback_origin("https://127.0.0.1:3443"));
        assert!(is_loopback_origin("http://[::1]:3000"));
        assert!(!is_loopback_origin("https://steward.example.com"));
        assert!(!is_loopback_origin("null"));
    }

    #[test]
    fn prune_removes_only_expired_terminal_history() {
        let db = Connection::open_in_memory().expect("open database");
        workflow_store::create_schema(&db).expect("create workflow schema");
        tool_registry::initialize(&db).expect("create registry schema");
        db.execute(
            "INSERT INTO workflow_runs VALUES ('old', 'old', 10, 0, 1, '', 'done', 0, 1, 0, 1, 1)",
            [],
        )
        .expect("insert old workflow");
        db.execute(
            "INSERT INTO workflow_runs VALUES ('active', 'active', 2, 0, 0.2, '', 'run', 0, 1, 0, 1, 1)",
            [],
        )
        .expect("insert active workflow");
        db.execute(
            "INSERT INTO tool_invocations (invocation_id, tool_id, approved, input_json, status, created_at) VALUES ('old-audit', 'memory.recall', 0, '{}', 'succeeded', 1)",
            [],
        )
        .expect("insert audit");

        let report = prune(
            &db,
            RetentionConfig {
                retention_days: 1,
                max_completed_workflows: 200,
            },
            200_000.0,
        )
        .expect("prune history");

        assert_eq!(report.tool_invocations, 1);
        assert_eq!(report.workflows, 1);
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM workflow_runs", [], |row| row
                .get::<_, i64>(0))
                .expect("count workflows"),
            1
        );
    }
}
