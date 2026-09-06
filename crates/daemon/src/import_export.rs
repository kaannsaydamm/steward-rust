//! Omega archive manifest + dry-run import (Tasks 27.1-27.3, §12.15).
//!
//! Archives carry config (secret refs only), the migrated DB, the Git-backed
//! context/harness bundles, and workflow plans. Plaintext secrets, keychain
//! material, process handles, and stale tokens are structurally excluded.

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

pub const ARCHIVE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArchiveManifest {
    pub archive_version: u32,
    pub source: String,
    pub created_at_ms: i64,
    /// Entity counts by type.
    pub entities: Value,
}

/// One dry-run finding.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub category: String, // conflict | skipped_secret | migration | entity
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DryRunReport {
    pub archive_version: u32,
    pub source: String,
    pub entities: Value,
    pub conflicts: Vec<String>,
    pub skipped_secrets: Vec<String>,
    pub required_migrations: Vec<i64>,
}

/// Builds a manifest for a directory prepared for export. Secret values are
/// excluded by construction: callers pass provider settings AFTER
/// `ProviderSettings::save` sanitized them (vault refs only).
pub fn build_manifest(source: &str, entities: Value) -> ArchiveManifest {
    ArchiveManifest {
        archive_version: ARCHIVE_VERSION,
        source: source.to_owned(),
        created_at_ms: now_ms(),
        entities,
    }
}

/// Dry-run import: validates the archive without touching the live install.
pub fn dry_run(archive_root: &Path) -> Result<DryRunReport> {
    let manifest_path = archive_root.join("manifest.json");
    let manifest: ArchiveManifest = serde_json::from_str(
        &std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("reading {}", manifest_path.display()))?,
    )
    .context("parsing manifest.json")?;

    let mut conflicts = Vec::new();
    let mut skipped_secrets = Vec::new();

    // Traversal defense: every referenced path must stay inside the archive.
    let canonical_root = archive_root.canonicalize().unwrap_or_else(|_| archive_root.to_path_buf());
    for entry in walk(archive_root)? {
        let canonical = entry.canonicalize().unwrap_or_else(|_| entry.clone());
        if !canonical.starts_with(&canonical_root) {
            conflicts.push(format!("archive entry escapes root: {}", entry.display()));
        }
    }

    // Secrets: vault references are reported as re-connect needed; raw keys
    // would be a conflict.
    let providers_path = archive_root.join("config/providers.json");
    if providers_path.exists() {
        let providers: Value = serde_json::from_str(&std::fs::read_to_string(&providers_path)?)?;
        if let Some(profiles) = providers.get("profiles").and_then(Value::as_array) {
            for profile in profiles {
                let profile_id = profile.get("profile_id").and_then(Value::as_str).unwrap_or("?");
                if let Some(secret_ref) = profile.get("secret_ref").and_then(Value::as_str) {
                    skipped_secrets.push(secret_ref.to_owned());
                } else if profile.get("legacy_api_key").and_then(Value::as_str).is_some()
                    || profile.get("api_key").and_then(Value::as_str).map(|k| !k.is_empty()).unwrap_or(false)
                {
                    conflicts.push(format!("profile {profile_id} carries an inline key; refusing import"));
                }
            }
        }
    }

    // Required migrations: archive DB schema vs current.
    let mut required_migrations = Vec::new();
    let db_path = archive_root.join("database/steward.db");
    if db_path.exists() {
        let connection = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        let version = crate::db_migrator::current_version(&connection).unwrap_or(0);
        for migration in crate::db_migrator::all_migrations() {
            if migration.version() > version {
                required_migrations.push(migration.version());
            }
        }
    }

    Ok(DryRunReport {
        archive_version: manifest.archive_version,
        source: manifest.source,
        entities: manifest.entities,
        conflicts,
        skipped_secrets,
        required_migrations,
    })
}

fn walk(root: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    fn inner(dir: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                inner(&path, files)?;
            } else {
                files.push(path);
            }
        }
        Ok(())
    }
    inner(root, &mut files)?;
    Ok(files)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn archive_with(providers_json: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("config")).unwrap();
        std::fs::write(
            temp.path().join("manifest.json"),
            json!({
                "archive_version": 1,
                "source": "steward-0.1.0",
                "created_at_ms": 1,
                "entities": {"threads": 2, "runs": 5}
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(temp.path().join("config/providers.json"), providers_json).unwrap();
        let root = temp.path().to_path_buf();
        (temp, root)
    }

    #[test]
    fn manifest_roundtrip_and_entities_reported() {
        let (_temp, root) = archive_with(r#"{"version":1,"profiles":[]}"#);
        let report = dry_run(&root).unwrap();
        assert_eq!(report.archive_version, 1);
        assert_eq!(report.source, "steward-0.1.0");
        assert_eq!(report.entities["threads"], 2);
        assert!(report.conflicts.is_empty());
    }

    #[test]
    fn vault_references_listed_as_reconnect_needed() {
        let providers = r#"{"version":1,"profiles":[{"profile_id":"p1","secret_ref":"steward://secret/provider/p1"}]}"#;
        let (_temp, root) = archive_with(providers);
        let report = dry_run(&root).unwrap();
        assert_eq!(
            report.skipped_secrets,
            vec!["steward://secret/provider/p1".to_owned()]
        );
        assert!(report.conflicts.is_empty(), "vault refs are not conflicts");
    }

    #[test]
    fn inline_keys_block_import() {
        let providers = r#"{"version":1,"profiles":[{"profile_id":"p1","legacy_api_key":"sk-live"}]}"#;
        let (_temp, root) = archive_with(providers);
        let report = dry_run(&root).unwrap();
        assert!(
            report.conflicts.iter().any(|c| c.contains("inline key")),
            "raw keys must block import: {:?}",
            report.conflicts
        );
    }

    #[test]
    fn required_migrations_reported_from_archive_db() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("database");
        std::fs::create_dir_all(&db_path).unwrap();
        {
            let mut connection = rusqlite::Connection::open(db_path.join("steward.db")).unwrap();
            crate::db_migrator::migrate(&mut connection, std::env::temp_dir().as_path()).unwrap();
            // Simulate an older archive: roll version back to 1.
            connection
                .execute("DELETE FROM schema_migrations WHERE version = 2", [])
                .unwrap();
        }
        std::fs::create_dir_all(temp.path().join("config")).unwrap();
        std::fs::write(
            temp.path().join("manifest.json"),
            json!({"archive_version": 1, "source": "s", "created_at_ms": 1, "entities": {}}).to_string(),
        )
        .unwrap();
        std::fs::write(temp.path().join("config/providers.json"), r#"{"version":1,"profiles":[]}"#).unwrap();

        let report = dry_run(temp.path()).unwrap();
        assert_eq!(report.required_migrations, vec![2], "v0002 must be re-run");
    }

    #[test]
    fn missing_manifest_is_an_error() {
        let temp = tempfile::tempdir().unwrap();
        assert!(dry_run(temp.path()).is_err());
    }

    #[test]
    fn archive_version_mismatch_is_surfaced() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("manifest.json"),
            json!({"archive_version": 99, "source": "future", "created_at_ms": 1, "entities": {}})
                .to_string(),
        )
        .unwrap();
        let report = dry_run(temp.path()).unwrap();
        assert_eq!(report.archive_version, 99, "caller decides compatibility");
    }
}
