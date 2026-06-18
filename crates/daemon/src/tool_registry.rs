use anyhow::{bail, Result};
use rusqlite::Connection;

#[path = "tool_registry_schema.rs"]
mod schema;
#[path = "tool_registry_seeds.rs"]
mod seeds;
pub(crate) use schema::initialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolRuntime {
    Builtin,
    Wasm,
    Process,
    Mcp,
}

impl ToolRuntime {
    const fn code(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Wasm => "wasm",
            Self::Process => "process",
            Self::Mcp => "mcp",
        }
    }

    fn parse(code: &str) -> Result<Self> {
        match code {
            "builtin" => Ok(Self::Builtin),
            "wasm" => Ok(Self::Wasm),
            "process" => Ok(Self::Process),
            "mcp" => Ok(Self::Mcp),
            _ => bail!("unknown tool runtime {code}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl RiskLevel {
    const fn code(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    fn parse(code: &str) -> Result<Self> {
        match code {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            _ => bail!("unknown risk level {code}"),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ToolDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub runtime: ToolRuntime,
    pub risk: RiskLevel,
    pub enabled: bool,
    pub requires_approval: bool,
}

#[derive(Debug, PartialEq)]
pub struct SkillDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
    pub tool_ids: Vec<String>,
    pub publisher_key: String,
    pub manifest_digest: String,
    pub installed_at: f64,
}

pub fn list_tools(connection: &Connection) -> Result<Vec<ToolDefinition>> {
    let mut statement = connection.prepare(
        "SELECT tool_id, name, description, runtime, risk_level, enabled, requires_approval
         FROM tools ORDER BY tool_id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, bool>(5)?,
            row.get::<_, bool>(6)?,
        ))
    })?;
    let mut tools = Vec::new();
    for row in rows {
        let (id, name, description, runtime, risk, enabled, requires_approval) = row?;
        tools.push(ToolDefinition {
            id,
            name,
            description,
            runtime: ToolRuntime::parse(&runtime)?,
            risk: RiskLevel::parse(&risk)?,
            enabled,
            requires_approval,
        });
    }
    Ok(tools)
}

pub fn get_tool(connection: &Connection, tool_id: &str) -> Result<Option<ToolDefinition>> {
    Ok(list_tools(connection)?
        .into_iter()
        .find(|tool| tool.id == tool_id))
}

pub fn list_skills(connection: &Connection) -> Result<Vec<SkillDefinition>> {
    let mut statement = connection.prepare(
        "SELECT skill_id, name, description, version, enabled,
                publisher_key, manifest_digest, installed_at
         FROM skills ORDER BY skill_id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, bool>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, f64>(7)?,
        ))
    })?;
    let mut skills = Vec::new();
    for row in rows {
        let (id, name, description, version, enabled, publisher_key, manifest_digest, installed_at) =
            row?;
        let tool_ids = skill_tool_ids(connection, &id)?;
        skills.push(SkillDefinition {
            id,
            name,
            description,
            version,
            enabled,
            tool_ids,
            publisher_key,
            manifest_digest,
            installed_at,
        });
    }
    Ok(skills)
}

pub fn get_skill(connection: &Connection, skill_id: &str) -> Result<Option<SkillDefinition>> {
    Ok(list_skills(connection)?
        .into_iter()
        .find(|skill| skill.id == skill_id))
}

fn skill_tool_ids(connection: &Connection, skill_id: &str) -> Result<Vec<String>> {
    let mut statement = connection
        .prepare("SELECT tool_id FROM skill_tools WHERE skill_id = ?1 ORDER BY position")?;
    let rows = statement.query_map([skill_id], |row| row.get(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

#[cfg(test)]
#[path = "tool_registry_tests.rs"]
mod tests;
