use anyhow::Result;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub id: String,
    pub distance: f32,
    pub payload: Option<String>,
}

#[derive(Clone)]
pub struct VectorStore {
    db: Arc<Mutex<Connection>>,
}

impl VectorStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Result<Self> {
        // Initialize virtual tables in a separate scope to avoid borrow issues
        {
            let conn = db.lock().unwrap();
            conn.execute_batch(
                "CREATE VIRTUAL TABLE IF NOT EXISTS vec_memories USING vec0(
                    memory_id TEXT PRIMARY KEY,
                    embedding FLOAT[384]
                );
                CREATE VIRTUAL TABLE IF NOT EXISTS vec_entities USING vec0(
                    entity_id TEXT PRIMARY KEY,
                    embedding FLOAT[384]
                );
                CREATE VIRTUAL TABLE IF NOT EXISTS vec_documents USING vec0(
                    doc_id TEXT PRIMARY KEY,
                    embedding FLOAT[384]
                );",
            )?;
        }

        Ok(Self { db })
    }

    pub fn store_memory_embedding(&self, memory_id: &str, embedding: &[f32]) -> Result<()> {
        let blob = vector_to_blob(embedding);
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT OR REPLACE INTO vec_memories (memory_id, embedding) VALUES (?1, ?2)",
            rusqlite::params![memory_id, blob],
        )?;
        Ok(())
    }

    pub fn store_entity_embedding(&self, entity_id: &str, embedding: &[f32]) -> Result<()> {
        let blob = vector_to_blob(embedding);
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT OR REPLACE INTO vec_entities (entity_id, embedding) VALUES (?1, ?2)",
            rusqlite::params![entity_id, blob],
        )?;
        Ok(())
    }

    pub fn search_memories(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let blob = vector_to_blob(query_embedding);
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT memory_id, distance FROM vec_memories WHERE embedding MATCH ?1 ORDER BY distance LIMIT ?2",
        )?;

        let results = stmt
            .query_map(rusqlite::params![blob, limit as i32], |row| {
                Ok(SearchResult {
                    id: row.get(0)?,
                    distance: row.get(1)?,
                    payload: None,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    pub fn search_entities(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let blob = vector_to_blob(query_embedding);
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT entity_id, distance FROM vec_entities WHERE embedding MATCH ?1 ORDER BY distance LIMIT ?2",
        )?;

        let results = stmt
            .query_map(rusqlite::params![blob, limit as i32], |row| {
                Ok(SearchResult {
                    id: row.get(0)?,
                    distance: row.get(1)?,
                    payload: None,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    pub fn delete_memory_embedding(&self, memory_id: &str) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "DELETE FROM vec_memories WHERE memory_id = ?1",
            rusqlite::params![memory_id],
        )?;
        Ok(())
    }
}

pub(crate) fn vector_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}
