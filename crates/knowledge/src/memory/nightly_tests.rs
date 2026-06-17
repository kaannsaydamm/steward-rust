use super::*;
use crate::vector::VectorStore;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

fn memory() -> AgentMemory {
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }
    let db = Arc::new(Mutex::new(Connection::open_in_memory().expect("open db")));
    {
        let conn = db.lock().expect("lock db");
        conn.execute_batch(
            "CREATE TABLE memories (
                id TEXT PRIMARY KEY,
                memory_type TEXT NOT NULL,
                content TEXT NOT NULL,
                metadata TEXT NOT NULL DEFAULT '{}',
                entities TEXT NOT NULL DEFAULT '[]',
                timestamp REAL NOT NULL
            );
            CREATE TABLE memory_lessons (
                id TEXT PRIMARY KEY,
                memory_id TEXT NOT NULL,
                scope TEXT NOT NULL,
                error_signature TEXT NOT NULL,
                correction TEXT NOT NULL,
                severity INTEGER NOT NULL DEFAULT 1,
                created_at REAL NOT NULL
            );
            CREATE TABLE nightly_dreams (
                id TEXT PRIMARY KEY,
                dream_date TEXT NOT NULL UNIQUE,
                memory_id TEXT NOT NULL,
                summary TEXT NOT NULL,
                positive_count INTEGER NOT NULL DEFAULT 0,
                negative_count INTEGER NOT NULL DEFAULT 0,
                reasoning_count INTEGER NOT NULL DEFAULT 0,
                created_at REAL NOT NULL
            );",
        )
        .expect("create schema");
    }
    let vector = VectorStore::new(db.clone()).expect("create vector store");
    AgentMemory::new(db, vector).expect("create memory")
}

#[test]
fn stores_negative_lesson_as_queryable_memory() {
    let memory = memory();
    let lesson = memory
        .store_negative_lesson(
            "cli",
            "stale binary in e2e",
            "build binaries before smoke",
            4,
        )
        .expect("store lesson");

    assert_eq!(lesson.severity, 4);
    let lessons = memory
        .recall_negative_lessons(Some("cli"), 10)
        .expect("recall lessons");
    assert_eq!(lessons.len(), 1);
    assert_eq!(lessons[0].memory_id, lesson.memory_id);

    let entries = memory
        .recall("", Some(MemoryType::Negative), 10)
        .expect("recall memory");
    assert_eq!(entries.len(), 1);
    assert!(entries[0].content.contains("stale binary"));
}

#[test]
fn creates_nightly_dream_report_with_counts() {
    let memory = memory();
    memory
        .store_negative_lesson("planner", "missed relational schema", "include DB map", 5)
        .expect("store lesson");
    memory
        .store(MemoryEntry {
            id: String::new(),
            memory_type: MemoryType::Reasoning,
            content: "reasoning trace".to_string(),
            metadata: HashMap::new(),
            entities: vec![],
            timestamp: now_unix_seconds().expect("now"),
        })
        .expect("store reasoning");

    let report = memory
        .create_nightly_dream("2026-06-16", 0.0, f64::MAX)
        .expect("dream report");

    assert_eq!(report.negative_count, 1);
    assert_eq!(report.reasoning_count, 1);
    assert!(report.summary.contains("Avoid repeats"));
}
