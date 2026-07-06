use anyhow::{bail, Context as _, Result};
use prost::Message;
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet, VecDeque};
use steward_core::pb::WorkflowDefinition;

const MAX_NODES: usize = 64;

pub fn create_schema(db: &Connection) -> Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS workflow_definitions (
            definition_id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            payload BLOB NOT NULL,
            created_at REAL NOT NULL,
            updated_at REAL NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_workflow_definitions_updated
            ON workflow_definitions(updated_at DESC);
        CREATE TABLE IF NOT EXISTS workflow_node_results (
            workflow_id TEXT NOT NULL,
            node_id TEXT NOT NULL,
            output TEXT NOT NULL,
            completed_at REAL NOT NULL,
            PRIMARY KEY(workflow_id, node_id),
            FOREIGN KEY(workflow_id) REFERENCES workflow_runs(workflow_id)
        );",
    )
    .context("creating workflow definition schema")?;
    Ok(())
}

pub fn validate_and_order(definition: &WorkflowDefinition) -> Result<Vec<String>> {
    if definition.name.trim().is_empty() {
        bail!("workflow definition name cannot be empty");
    }
    if definition.nodes.is_empty() || definition.nodes.len() > MAX_NODES {
        bail!("workflow definition must contain 1 to {MAX_NODES} nodes");
    }
    let mut ids = HashSet::new();
    for node in &definition.nodes {
        if !valid_id(&node.node_id) {
            bail!("workflow node id '{}' is invalid", node.node_id);
        }
        if !ids.insert(node.node_id.clone()) {
            bail!("workflow node id '{}' is duplicated", node.node_id);
        }
        if node.title.trim().is_empty() || node.instruction.trim().is_empty() {
            bail!(
                "workflow node '{}' needs a title and instruction",
                node.node_id
            );
        }
    }

    let mut indegree = ids
        .iter()
        .map(|id| (id.clone(), 0_usize))
        .collect::<HashMap<_, _>>();
    let mut outgoing = HashMap::<String, Vec<String>>::new();
    let mut edges = HashSet::new();
    for edge in &definition.connections {
        if !ids.contains(&edge.source_node_id) || !ids.contains(&edge.target_node_id) {
            bail!("workflow connection references an unknown node");
        }
        if edge.source_node_id == edge.target_node_id {
            bail!(
                "workflow node '{}' cannot connect to itself",
                edge.source_node_id
            );
        }
        if !edges.insert((edge.source_node_id.clone(), edge.target_node_id.clone())) {
            bail!("workflow connection is duplicated");
        }
        outgoing
            .entry(edge.source_node_id.clone())
            .or_default()
            .push(edge.target_node_id.clone());
        *indegree
            .get_mut(&edge.target_node_id)
            .expect("validated node") += 1;
    }

    let mut ready = definition
        .nodes
        .iter()
        .filter(|node| indegree[&node.node_id] == 0)
        .map(|node| node.node_id.clone())
        .collect::<VecDeque<_>>();
    let mut order = Vec::with_capacity(definition.nodes.len());
    while let Some(id) = ready.pop_front() {
        order.push(id.clone());
        for target in outgoing.get(&id).into_iter().flatten() {
            let degree = indegree.get_mut(target).expect("validated node");
            *degree -= 1;
            if *degree == 0 {
                ready.push_back(target.clone());
            }
        }
    }
    if order.len() != definition.nodes.len() {
        bail!("workflow definition contains a cycle");
    }
    Ok(order)
}

pub fn save(db: &Connection, mut definition: WorkflowDefinition) -> Result<WorkflowDefinition> {
    validate_and_order(&definition)?;
    let now = crate::workflow_events::current_unix_seconds();
    if definition.definition_id.is_empty() {
        definition.definition_id = uuid::Uuid::new_v4().to_string();
    } else if !valid_id(&definition.definition_id) {
        bail!("workflow definition id is invalid");
    }
    let existing_created = db
        .query_row(
            "SELECT created_at FROM workflow_definitions WHERE definition_id = ?1",
            [&definition.definition_id],
            |row| row.get::<_, f64>(0),
        )
        .ok();
    definition.created_at = existing_created.unwrap_or(now);
    definition.updated_at = now;
    let payload = definition.encode_to_vec();
    db.execute(
        "INSERT INTO workflow_definitions (definition_id, name, payload, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(definition_id) DO UPDATE SET
            name = excluded.name, payload = excluded.payload, updated_at = excluded.updated_at",
        params![
            definition.definition_id,
            definition.name,
            payload,
            definition.created_at,
            definition.updated_at,
        ],
    )
    .context("saving workflow definition")?;
    Ok(definition)
}

pub fn get(db: &Connection, definition_id: &str) -> Result<Option<WorkflowDefinition>> {
    let payload = db
        .query_row(
            "SELECT payload FROM workflow_definitions WHERE definition_id = ?1",
            [definition_id],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .ok();
    payload
        .map(|bytes| {
            WorkflowDefinition::decode(bytes.as_slice()).context("decoding workflow definition")
        })
        .transpose()
}

pub fn list(db: &Connection) -> Result<Vec<WorkflowDefinition>> {
    let mut statement = db.prepare(
        "SELECT payload FROM workflow_definitions ORDER BY updated_at DESC, definition_id",
    )?;
    let rows = statement.query_map([], |row| row.get::<_, Vec<u8>>(0))?;
    rows.map(|row| {
        WorkflowDefinition::decode(row?.as_slice())
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
    })
    .collect::<std::result::Result<Vec<_>, _>>()
    .context("listing workflow definitions")
}

pub fn delete(db: &Connection, definition_id: &str) -> Result<bool> {
    Ok(db.execute(
        "DELETE FROM workflow_definitions WHERE definition_id = ?1",
        [definition_id],
    )? > 0)
}

pub fn load_results(db: &Connection, workflow_id: &str) -> Result<HashMap<String, String>> {
    let mut statement =
        db.prepare("SELECT node_id, output FROM workflow_node_results WHERE workflow_id = ?1")?;
    let rows = statement.query_map([workflow_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.collect::<std::result::Result<HashMap<_, _>, _>>()
        .context("loading workflow node results")
}

pub fn save_result(db: &Connection, workflow_id: &str, node_id: &str, output: &str) -> Result<()> {
    db.execute(
        "INSERT INTO workflow_node_results (workflow_id, node_id, output, completed_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(workflow_id, node_id) DO UPDATE SET
            output = excluded.output, completed_at = excluded.completed_at",
        params![
            workflow_id,
            node_id,
            output,
            crate::workflow_events::current_unix_seconds()
        ],
    )?;
    Ok(())
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[cfg(test)]
#[path = "workflow_definition_tests.rs"]
mod tests;
