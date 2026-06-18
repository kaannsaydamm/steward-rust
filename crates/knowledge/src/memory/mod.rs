use crate::vector::VectorStore;
use anyhow::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

mod nightly;
pub use nightly::{DreamReport, NegativeLesson};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum MemoryType {
    ShortTerm,
    LongTerm,
    Reasoning,
    Negative,
    Dream,
}

impl MemoryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryType::ShortTerm => "short_term",
            MemoryType::LongTerm => "long_term",
            MemoryType::Reasoning => "reasoning",
            MemoryType::Negative => "negative",
            MemoryType::Dream => "dream",
        }
    }

    pub fn from_code(s: &str) -> Self {
        match s {
            "short_term" => MemoryType::ShortTerm,
            "long_term" => MemoryType::LongTerm,
            "reasoning" => MemoryType::Reasoning,
            "negative" => MemoryType::Negative,
            "dream" => MemoryType::Dream,
            _ => MemoryType::ShortTerm,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub memory_type: MemoryType,
    pub content: String,
    pub metadata: HashMap<String, String>,
    pub entities: Vec<String>,
    pub timestamp: f64,
}

pub struct AgentMemory {
    pub(super) db: Arc<Mutex<Connection>>,
    vector: VectorStore,
}

impl AgentMemory {
    pub fn new(db: Arc<Mutex<Connection>>, vector: VectorStore) -> Result<Self> {
        Ok(Self { db, vector })
    }

    pub fn store(&self, entry: MemoryEntry) -> Result<String> {
        let id = if entry.id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            entry.id.clone()
        };

        let mut entry = entry;
        entry.id = id.clone();

        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO memories (id, memory_type, content, metadata, entities, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                entry.id,
                entry.memory_type.as_str(),
                entry.content,
                serde_json::to_string(&entry.metadata)?,
                serde_json::to_string(&entry.entities)?,
                entry.timestamp,
            ],
        )?;

        Ok(id)
    }

    pub fn store_with_embedding(&self, entry: MemoryEntry, embedding: &[f32]) -> Result<String> {
        let id = self.store(entry)?;
        if embedding.len() >= 128 {
            self.vector.store_memory_embedding(&id, embedding)?;
        }
        Ok(id)
    }

    pub fn recall(
        &self,
        query: &str,
        memory_type: Option<MemoryType>,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>> {
        let mut sql = String::from(
            "SELECT id, memory_type, content, metadata, entities, timestamp FROM memories WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref mt) = memory_type {
            sql.push_str(&format!(" AND memory_type = ?{}", params.len() + 1));
            params.push(Box::new(mt.as_str().to_string()));
        }

        if !query.trim().is_empty() {
            sql.push_str(&format!(
                " AND id IN (SELECT id FROM memories_fts WHERE memories_fts MATCH ?{})",
                params.len() + 1
            ));
            let sanitized: String = query
                .chars()
                .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                .collect();
            let fts_query = sanitized
                .split_whitespace()
                .map(|w| format!("{}*", w))
                .collect::<Vec<_>>()
                .join(" AND ");
            params.push(Box::new(fts_query));
        }

        sql.push_str(&format!(
            " ORDER BY timestamp DESC LIMIT ?{}",
            params.len() + 1
        ));
        params.push(Box::new(limit as i32));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();

        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(&sql)?;
        let results = stmt
            .query_map(param_refs.as_slice(), |row| {
                let metadata_str: String = row.get(3)?;
                let entities_str: String = row.get(4)?;
                Ok(MemoryEntry {
                    id: row.get(0)?,
                    memory_type: MemoryType::from_code(&row.get::<_, String>(1)?),
                    content: row.get(2)?,
                    metadata: serde_json::from_str(&metadata_str).unwrap_or_default(),
                    entities: serde_json::from_str(&entities_str).unwrap_or_default(),
                    timestamp: row.get(5)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    pub fn get_conversation(&self, session_id: &str) -> Result<Vec<MemoryEntry>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT m.id, m.memory_type, m.content, m.metadata, m.entities, m.timestamp
             FROM memories m
             JOIN memory_sessions ms ON m.id = ms.memory_id
             WHERE ms.session_id = ?1 AND m.memory_type = 'short_term'
             ORDER BY m.timestamp ASC",
        )?;

        let results = stmt
            .query_map(rusqlite::params![session_id], |row| {
                let metadata_str: String = row.get(3)?;
                let entities_str: String = row.get(4)?;
                Ok(MemoryEntry {
                    id: row.get(0)?,
                    memory_type: MemoryType::from_code(&row.get::<_, String>(1)?),
                    content: row.get(2)?,
                    metadata: serde_json::from_str(&metadata_str).unwrap_or_default(),
                    entities: serde_json::from_str(&entities_str).unwrap_or_default(),
                    timestamp: row.get(5)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    pub fn store_reasoning(&self, workflow_id: &str, agent_id: &str, trace: &str) -> Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();

        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO reasoning_traces (id, workflow_id, agent_id, trace, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, workflow_id, agent_id, trace, now],
        )?;

        Ok(())
    }

    pub fn store_reasoning_memory(
        &self,
        workflow_id: &str,
        agent_id: &str,
        trace: &str,
    ) -> Result<String> {
        let entry = MemoryEntry {
            id: String::new(),
            memory_type: MemoryType::Reasoning,
            content: trace.to_string(),
            metadata: {
                let mut m = HashMap::new();
                m.insert("workflow_id".to_string(), workflow_id.to_string());
                m.insert("agent_id".to_string(), agent_id.to_string());
                m
            },
            entities: vec![],
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs_f64(),
        };
        self.store(entry)
    }

    pub fn recall_reasoning(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
        self.recall(query, Some(MemoryType::Reasoning), limit)
    }

    pub fn consolidate(&self) -> Result<usize> {
        let db = self.db.lock().unwrap();
        let promoted = db.execute(
            "UPDATE memories SET memory_type = 'long_term'
             WHERE memory_type = 'short_term'
             AND json_extract(metadata, '$.important') = 'true'",
            [],
        )?;
        Ok(promoted)
    }
}
