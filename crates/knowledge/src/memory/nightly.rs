use super::{AgentMemory, MemoryEntry, MemoryType};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NegativeLesson {
    pub id: String,
    pub memory_id: String,
    pub scope: String,
    pub error_signature: String,
    pub correction: String,
    pub severity: i32,
    pub created_at: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DreamReport {
    pub id: String,
    pub dream_date: String,
    pub memory_id: String,
    pub summary: String,
    pub positive_count: i32,
    pub negative_count: i32,
    pub reasoning_count: i32,
    pub created_at: f64,
}

impl AgentMemory {
    pub fn store_negative_lesson(
        &self,
        scope: &str,
        error_signature: &str,
        correction: &str,
        severity: i32,
    ) -> Result<NegativeLesson> {
        let lesson_id = uuid::Uuid::new_v4().to_string();
        let now = now_unix_seconds()?;
        let severity = severity.clamp(1, 5);
        let content = format!("Mistake in {scope}: {error_signature}. Correction: {correction}");
        let mut metadata = HashMap::new();
        metadata.insert("scope".to_string(), scope.to_string());
        metadata.insert("error_signature".to_string(), error_signature.to_string());
        metadata.insert("correction".to_string(), correction.to_string());
        metadata.insert("severity".to_string(), severity.to_string());
        let memory_id = self.store(MemoryEntry {
            id: String::new(),
            memory_type: MemoryType::Negative,
            content,
            metadata,
            entities: vec![scope.to_string()],
            timestamp: now,
        })?;

        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO memory_lessons
             (id, memory_id, scope, error_signature, correction, severity, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                lesson_id,
                memory_id,
                scope,
                error_signature,
                correction,
                severity,
                now
            ],
        )?;

        Ok(NegativeLesson {
            id: lesson_id,
            memory_id,
            scope: scope.to_string(),
            error_signature: error_signature.to_string(),
            correction: correction.to_string(),
            severity,
            created_at: now,
        })
    }

    pub fn recall_negative_lessons(
        &self,
        scope: Option<&str>,
        limit: usize,
    ) -> Result<Vec<NegativeLesson>> {
        let db = self.db.lock().unwrap();
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let map_row = |row: &rusqlite::Row<'_>| {
            Ok(NegativeLesson {
                id: row.get(0)?,
                memory_id: row.get(1)?,
                scope: row.get(2)?,
                error_signature: row.get(3)?,
                correction: row.get(4)?,
                severity: row.get(5)?,
                created_at: row.get(6)?,
            })
        };

        let lessons = if let Some(scope) = scope {
            let mut stmt = db.prepare(
                "SELECT id, memory_id, scope, error_signature, correction, severity, created_at
                 FROM memory_lessons
                 WHERE scope = ?1
                 ORDER BY severity DESC, created_at DESC
                 LIMIT ?2",
            )?;
            let rows = stmt
                .query_map(rusqlite::params![scope, limit], map_row)?
                .filter_map(|row| row.ok())
                .collect();
            rows
        } else {
            let mut stmt = db.prepare(
                "SELECT id, memory_id, scope, error_signature, correction, severity, created_at
                 FROM memory_lessons
                 ORDER BY severity DESC, created_at DESC
                 LIMIT ?1",
            )?;
            let rows = stmt
                .query_map(rusqlite::params![limit], map_row)?
                .filter_map(|row| row.ok())
                .collect();
            rows
        };
        Ok(lessons)
    }

    pub fn create_nightly_dream(
        &self,
        dream_date: &str,
        window_start: f64,
        window_end: f64,
    ) -> Result<DreamReport> {
        let positive_count = self.count_memories(MemoryType::LongTerm, window_start, window_end)?;
        let negative_count = self.count_memories(MemoryType::Negative, window_start, window_end)?;
        let reasoning_count =
            self.count_memories(MemoryType::Reasoning, window_start, window_end)?;
        let negative_lessons = self.recall_negative_lessons(None, 5)?;
        let summary = build_dream_summary(
            dream_date,
            positive_count,
            negative_count,
            reasoning_count,
            &negative_lessons,
        );
        let now = now_unix_seconds()?;
        let mut metadata = HashMap::new();
        metadata.insert("dream_date".to_string(), dream_date.to_string());
        metadata.insert("positive_count".to_string(), positive_count.to_string());
        metadata.insert("negative_count".to_string(), negative_count.to_string());
        metadata.insert("reasoning_count".to_string(), reasoning_count.to_string());
        let memory_id = self.store(MemoryEntry {
            id: String::new(),
            memory_type: MemoryType::Dream,
            content: summary.clone(),
            metadata,
            entities: vec!["nightly_dream".to_string()],
            timestamp: now,
        })?;
        let dream_id = uuid::Uuid::new_v4().to_string();

        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO nightly_dreams
             (id, dream_date, memory_id, summary, positive_count, negative_count, reasoning_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(dream_date) DO UPDATE SET
                memory_id = excluded.memory_id,
                summary = excluded.summary,
                positive_count = excluded.positive_count,
                negative_count = excluded.negative_count,
                reasoning_count = excluded.reasoning_count,
                created_at = excluded.created_at",
            rusqlite::params![
                dream_id,
                dream_date,
                memory_id,
                summary,
                positive_count,
                negative_count,
                reasoning_count,
                now
            ],
        )?;

        Ok(DreamReport {
            id: dream_id,
            dream_date: dream_date.to_string(),
            memory_id,
            summary,
            positive_count,
            negative_count,
            reasoning_count,
            created_at: now,
        })
    }

    fn count_memories(
        &self,
        memory_type: MemoryType,
        window_start: f64,
        window_end: f64,
    ) -> Result<i32> {
        let db = self.db.lock().unwrap();
        let count = db.query_row(
            "SELECT COUNT(*) FROM memories
             WHERE memory_type = ?1 AND timestamp >= ?2 AND timestamp < ?3",
            rusqlite::params![memory_type.as_str(), window_start, window_end],
            |row| row.get::<_, i32>(0),
        )?;
        Ok(count)
    }
}

fn now_unix_seconds() -> Result<f64> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs_f64())
}

fn build_dream_summary(
    dream_date: &str,
    positive_count: i32,
    negative_count: i32,
    reasoning_count: i32,
    negative_lessons: &[NegativeLesson],
) -> String {
    let mut summary = format!(
        "Nightly dream {dream_date}: consolidated {positive_count} long-term memories, {negative_count} negative lessons, and {reasoning_count} reasoning traces."
    );
    if !negative_lessons.is_empty() {
        let lessons = negative_lessons
            .iter()
            .map(|lesson| {
                format!(
                    "[{}:{} -> {}]",
                    lesson.scope, lesson.error_signature, lesson.correction
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        summary.push_str(" Avoid repeats: ");
        summary.push_str(&lessons);
    }
    summary
}

#[cfg(test)]
#[path = "nightly_tests.rs"]
mod tests;
