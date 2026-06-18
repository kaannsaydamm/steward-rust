#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterConfig {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredTool {
    pub name: String,
    pub description: String,
    pub input_schema_json: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolBinding {
    pub adapter_id: String,
    pub remote_name: String,
}

pub fn register(connection: &Connection, config: &AdapterConfig) -> Result<()> {
    validate_config(config)?;
    let args_json = serde_json::to_string(&config.args).context("serializing MCP arguments")?;
    connection.execute(
        "INSERT INTO mcp_adapters (adapter_id, name, command, args_json, cwd)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(adapter_id) DO UPDATE SET
            name = excluded.name,
            command = excluded.command,
            args_json = excluded.args_json,
            cwd = excluded.cwd",
        params![
            config.id,
            config.name,
            config.command,
            args_json,
            config.cwd
        ],
    )?;
    Ok(())
}

pub fn list(connection: &Connection) -> Result<Vec<AdapterConfig>> {
    let mut statement = connection.prepare(
        "SELECT adapter_id, name, command, args_json, cwd
         FROM mcp_adapters ORDER BY adapter_id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;
    rows.map(|row| {
        let (id, name, command, args_json, cwd) = row?;
        Ok(AdapterConfig {
            id,
            name,
            command,
            args: serde_json::from_str(&args_json).context("parsing stored MCP arguments")?,
            cwd,
        })
    })
    .collect()
}

pub fn get(connection: &Connection, adapter_id: &str) -> Result<Option<AdapterConfig>> {
    Ok(list(connection)?
        .into_iter()
        .find(|adapter| adapter.id == adapter_id))
}

pub fn replace_tools(
    connection: &Connection,
    adapter_id: &str,
    tools: &[DiscoveredTool],
) -> Result<Vec<String>> {
    if get(connection, adapter_id)?.is_none() {
        bail!("MCP adapter '{adapter_id}' is not registered");
    }
    let transaction = connection.unchecked_transaction()?;
    retire_adapter_tools(&transaction, adapter_id)?;
    let mut tool_ids = Vec::with_capacity(tools.len());
    for tool in tools {
        if tool.name.trim().is_empty() {
            bail!("MCP tool name cannot be empty");
        }
        let tool_id = dynamic_tool_id(adapter_id, &tool.name);
        let display_name = format!("{adapter_id}: {}", tool.name);
        transaction.execute(
            "INSERT INTO tools
             (tool_id, name, description, runtime, risk_level, enabled, requires_approval)
             VALUES (?1, ?2, ?3, 'mcp', 'medium', 1, 1)
             ON CONFLICT(tool_id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                runtime = 'mcp',
                risk_level = 'medium',
                enabled = 1,
                requires_approval = 1",
            params![tool_id, display_name, tool.description],
        )?;
        transaction.execute(
            "INSERT INTO mcp_tools (tool_id, adapter_id, remote_name, input_schema_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![tool_id, adapter_id, tool.name, tool.input_schema_json],
        )?;
        tool_ids.push(tool_id);
    }
    transaction.commit()?;
    Ok(tool_ids)
}

pub fn binding(connection: &Connection, tool_id: &str) -> Result<Option<ToolBinding>> {
    let mut statement =
        connection.prepare("SELECT adapter_id, remote_name FROM mcp_tools WHERE tool_id = ?1")?;
    let mut rows = statement.query([tool_id])?;
    match rows.next()? {
        Some(row) => Ok(Some(ToolBinding {
            adapter_id: row.get(0)?,
            remote_name: row.get(1)?,
        })),
        None => Ok(None),
    }
}

pub fn set_tools_enabled(connection: &Connection, adapter_id: &str, enabled: bool) -> Result<()> {
    connection.execute(
        "UPDATE tools SET enabled = ?2
         WHERE tool_id IN (SELECT tool_id FROM mcp_tools WHERE adapter_id = ?1)",
        params![adapter_id, enabled],
    )?;
    Ok(())
}

pub fn disable_all_tools(connection: &Connection) -> Result<()> {
    connection.execute("UPDATE tools SET enabled = 0 WHERE runtime = 'mcp'", [])?;
    Ok(())
}

pub fn remove(connection: &Connection, adapter_id: &str) -> Result<bool> {
    let transaction = connection.unchecked_transaction()?;
    retire_adapter_tools(&transaction, adapter_id)?;
    let removed = transaction.execute(
        "DELETE FROM mcp_adapters WHERE adapter_id = ?1",
        [adapter_id],
    )? > 0;
    transaction.commit()?;
    Ok(removed)
}

fn retire_adapter_tools(connection: &Connection, adapter_id: &str) -> Result<()> {
    let mut statement = connection
        .prepare("SELECT tool_id FROM mcp_tools WHERE adapter_id = ?1 ORDER BY tool_id")?;
    let tool_ids = statement
        .query_map([adapter_id], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);
    connection.execute("DELETE FROM mcp_tools WHERE adapter_id = ?1", [adapter_id])?;
    for tool_id in tool_ids {
        connection.execute(
            "DELETE FROM tools WHERE tool_id = ?1
             AND NOT EXISTS (SELECT 1 FROM tool_invocations WHERE tool_id = ?1)",
            [&tool_id],
        )?;
        connection.execute(
            "UPDATE tools SET enabled = 0 WHERE tool_id = ?1",
            [&tool_id],
        )?;
    }
    Ok(())
}

fn validate_config(config: &AdapterConfig) -> Result<()> {
    if config.id.trim().is_empty()
        || config.name.trim().is_empty()
        || config.command.trim().is_empty()
    {
        bail!("MCP adapter id, name, and command are required");
    }
    if !config
        .id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
    {
        bail!("MCP adapter id may contain only ASCII letters, digits, '.', '-', and '_'");
    }
    Ok(())
}

fn dynamic_tool_id(adapter_id: &str, remote_name: &str) -> String {
    let slug = remote_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .take(32)
        .collect::<String>();
    let digest = hex::encode(Sha256::digest(remote_name.as_bytes()));
    format!("mcp.{adapter_id}.{slug}.{}", &digest[..8])
}

#[cfg(test)]
#[path = "mcp_registry_tests.rs"]
mod tests;
use anyhow::{bail, Context as _, Result};
use rusqlite::{params, Connection};
use sha2::{Digest as _, Sha256};
