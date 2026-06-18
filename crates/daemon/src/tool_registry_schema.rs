use super::seeds::{SKILLS, TOOLS};
use anyhow::{Context, Result};
use rusqlite::{params, Connection};

pub(crate) fn initialize(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS tools (
            tool_id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE,
            description TEXT NOT NULL,
            runtime TEXT NOT NULL CHECK(runtime IN ('builtin', 'wasm', 'process', 'mcp')),
            risk_level TEXT NOT NULL CHECK(risk_level IN ('low', 'medium', 'high')),
            enabled INTEGER NOT NULL CHECK(enabled IN (0, 1)),
            requires_approval INTEGER NOT NULL CHECK(requires_approval IN (0, 1))
         );
         CREATE TABLE IF NOT EXISTS skills (
            skill_id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE,
            description TEXT NOT NULL, version TEXT NOT NULL,
            enabled INTEGER NOT NULL CHECK(enabled IN (0, 1)),
            publisher_key TEXT NOT NULL DEFAULT '',
            signature TEXT NOT NULL DEFAULT '',
            manifest_digest TEXT NOT NULL DEFAULT '',
            installed_at REAL NOT NULL DEFAULT 0
         );
         CREATE TABLE IF NOT EXISTS skill_tools (
            skill_id TEXT NOT NULL, tool_id TEXT NOT NULL,
            position INTEGER NOT NULL CHECK(position >= 0),
            required INTEGER NOT NULL CHECK(required IN (0, 1)),
            PRIMARY KEY(skill_id, tool_id),
            FOREIGN KEY(skill_id) REFERENCES skills(skill_id) ON DELETE CASCADE,
            FOREIGN KEY(tool_id) REFERENCES tools(tool_id) ON DELETE RESTRICT
         );
         CREATE INDEX IF NOT EXISTS idx_skill_tools_tool_id
            ON skill_tools(tool_id);
         CREATE TABLE IF NOT EXISTS tool_invocations (
            sequence INTEGER PRIMARY KEY AUTOINCREMENT,
            invocation_id TEXT NOT NULL UNIQUE,
            tool_id TEXT NOT NULL,
            approved INTEGER NOT NULL CHECK(approved IN (0, 1)),
            input_json TEXT NOT NULL,
            status TEXT NOT NULL CHECK(status IN (
                'pending_approval', 'denied', 'succeeded', 'failed'
            )),
            output TEXT NOT NULL DEFAULT '',
            error TEXT NOT NULL DEFAULT '',
            created_at REAL NOT NULL,
            FOREIGN KEY(tool_id) REFERENCES tools(tool_id) ON DELETE RESTRICT
         );
         CREATE INDEX IF NOT EXISTS idx_tool_invocations_tool_sequence
            ON tool_invocations(tool_id, sequence DESC);
         CREATE TABLE IF NOT EXISTS mcp_adapters (
            adapter_id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            command TEXT NOT NULL,
            args_json TEXT NOT NULL,
            cwd TEXT
         );
         CREATE TABLE IF NOT EXISTS mcp_tools (
            tool_id TEXT PRIMARY KEY,
            adapter_id TEXT NOT NULL,
            remote_name TEXT NOT NULL,
            input_schema_json TEXT NOT NULL,
            UNIQUE(adapter_id, remote_name),
            FOREIGN KEY(tool_id) REFERENCES tools(tool_id) ON DELETE CASCADE,
            FOREIGN KEY(adapter_id) REFERENCES mcp_adapters(adapter_id) ON DELETE CASCADE
         );
         CREATE INDEX IF NOT EXISTS idx_mcp_tools_adapter
            ON mcp_tools(adapter_id);",
    )?;
    add_skill_provenance_columns(connection);
    let transaction = connection.unchecked_transaction()?;
    for tool in TOOLS {
        transaction.execute(
            "INSERT INTO tools
             (tool_id, name, description, runtime, risk_level, enabled, requires_approval)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(tool_id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                runtime = excluded.runtime,
                risk_level = excluded.risk_level,
                requires_approval = excluded.requires_approval",
            params![
                tool.id,
                tool.name,
                tool.description,
                tool.runtime.code(),
                tool.risk.code(),
                tool.enabled,
                tool.requires_approval
            ],
        )?;
    }
    for skill in SKILLS {
        transaction.execute(
            "INSERT INTO skills
             (skill_id, name, description, version, enabled)
             VALUES (?1, ?2, ?3, '1.0.0', ?4)
             ON CONFLICT(skill_id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                version = excluded.version",
            params![skill.id, skill.name, skill.description, skill.enabled],
        )?;
        for (position, tool_id) in skill.tools.iter().enumerate() {
            transaction.execute(
                "INSERT INTO skill_tools (skill_id, tool_id, position, required)
                 VALUES (?1, ?2, ?3, 1)
                 ON CONFLICT(skill_id, tool_id) DO UPDATE SET
                    position = excluded.position,
                    required = excluded.required",
                params![skill.id, tool_id, position],
            )?;
        }
    }
    transaction.commit().context("committing registry seeds")?;
    Ok(())
}

fn add_skill_provenance_columns(connection: &Connection) {
    for statement in [
        "ALTER TABLE skills ADD COLUMN publisher_key TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE skills ADD COLUMN signature TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE skills ADD COLUMN manifest_digest TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE skills ADD COLUMN installed_at REAL NOT NULL DEFAULT 0",
    ] {
        let _ = connection.execute(statement, []);
    }
}
