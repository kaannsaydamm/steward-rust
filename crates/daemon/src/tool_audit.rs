use anyhow::{bail, Result};
use rusqlite::{params, Connection};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvocationStatus {
    PendingApproval,
    Denied,
    Succeeded,
    Failed,
}

impl InvocationStatus {
    const fn code(self) -> &'static str {
        match self {
            Self::PendingApproval => "pending_approval",
            Self::Denied => "denied",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }

    fn parse(code: &str) -> Result<Self> {
        match code {
            "pending_approval" => Ok(Self::PendingApproval),
            "denied" => Ok(Self::Denied),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            _ => bail!("unknown invocation status {code}"),
        }
    }
}

pub struct NewInvocation<'a> {
    pub tool_id: &'a str,
    pub approved: bool,
    pub input_json: &'a str,
    pub status: InvocationStatus,
    pub output: &'a str,
    pub error: &'a str,
}

#[derive(Debug, PartialEq)]
pub struct InvocationRecord {
    pub invocation_id: String,
    pub tool_id: String,
    pub approved: bool,
    pub input_json: String,
    pub status: InvocationStatus,
    pub output: String,
    pub error: String,
    pub created_at: f64,
}

pub fn record(connection: &Connection, invocation: NewInvocation<'_>) -> Result<String> {
    let invocation_id = uuid::Uuid::new_v4().to_string();
    connection.execute(
        "INSERT INTO tool_invocations (
            invocation_id, tool_id, approved, input_json, status, output, error, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            invocation_id,
            invocation.tool_id,
            invocation.approved,
            invocation.input_json,
            invocation.status.code(),
            invocation.output,
            invocation.error,
            unix_seconds(),
        ],
    )?;
    Ok(invocation_id)
}

pub fn list_recent(connection: &Connection, limit: usize) -> Result<Vec<InvocationRecord>> {
    let limit = limit.clamp(1, 100);
    let mut statement = connection.prepare(
        "SELECT invocation_id, tool_id, approved, input_json, status, output, error, created_at
         FROM tool_invocations ORDER BY sequence DESC LIMIT ?1",
    )?;
    let rows = statement.query_map([limit], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, bool>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, f64>(7)?,
        ))
    })?;
    let mut records = Vec::new();
    for row in rows {
        let (invocation_id, tool_id, approved, input_json, status, output, error, created_at) =
            row?;
        records.push(InvocationRecord {
            invocation_id,
            tool_id,
            approved,
            input_json,
            status: InvocationStatus::parse(&status)?,
            output,
            error,
            created_at,
        });
    }
    Ok(records)
}

fn unix_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}

#[cfg(test)]
#[path = "tool_audit_tests.rs"]
mod tests;
