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
            enabled INTEGER NOT NULL CHECK(enabled IN (0, 1))
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
            ON skill_tools(tool_id);",
    )?;
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
             VALUES (?1, ?2, ?3, '1.0.0', 1)
             ON CONFLICT(skill_id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                version = excluded.version",
            params![skill.id, skill.name, skill.description],
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
