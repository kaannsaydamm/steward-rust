pub mod graph;
pub mod hybrid;
pub mod memory;
pub mod vector;

use anyhow::Result;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub use graph::{GraphEdge, GraphNode, GraphStore, KnowledgeGraph};
pub use hybrid::{GraphRagResult, HybridSearch, ScoredResult};
pub use memory::{AgentMemory, MemoryEntry, MemoryType};
pub use vector::{SearchResult, VectorStore};

/// Master engine that ties together graph, vector, hybrid search, and agent memory.
pub struct KnowledgeEngine {
    pub graph: Arc<Mutex<GraphStore>>,
    pub vector: VectorStore,
    pub hybrid: HybridSearch,
    pub memory: AgentMemory,
    pub db: Arc<Mutex<Connection>>,
}

impl KnowledgeEngine {
    /// Initialize the knowledge engine with an existing SQLite connection.
    /// The connection must have sqlite-vec already initialized.
    pub fn new(db: Connection) -> Result<Self> {
        let db = Arc::new(Mutex::new(db));

        // Create all necessary tables
        {
            let conn = db.lock().unwrap();
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS kg_nodes (
                    id TEXT PRIMARY KEY,
                    label TEXT NOT NULL,
                    node_type TEXT NOT NULL DEFAULT 'entity',
                    properties TEXT NOT NULL DEFAULT '{}'
                );
                CREATE TABLE IF NOT EXISTS kg_edges (
                    id TEXT PRIMARY KEY,
                    source_id TEXT NOT NULL,
                    target_id TEXT NOT NULL,
                    relationship TEXT NOT NULL,
                    properties TEXT NOT NULL DEFAULT '{}',
                    weight REAL NOT NULL DEFAULT 1.0,
                    FOREIGN KEY (source_id) REFERENCES kg_nodes(id),
                    FOREIGN KEY (target_id) REFERENCES kg_nodes(id)
                );
                CREATE TABLE IF NOT EXISTS memories (
                    id TEXT PRIMARY KEY,
                    memory_type TEXT NOT NULL,
                    content TEXT NOT NULL,
                    metadata TEXT NOT NULL DEFAULT '{}',
                    entities TEXT NOT NULL DEFAULT '[]',
                    timestamp REAL NOT NULL
                );
                CREATE TABLE IF NOT EXISTS memory_sessions (
                    session_id TEXT NOT NULL,
                    memory_id TEXT NOT NULL,
                    FOREIGN KEY (memory_id) REFERENCES memories(id)
                );
                CREATE TABLE IF NOT EXISTS reasoning_traces (
                    id TEXT PRIMARY KEY,
                    workflow_id TEXT NOT NULL,
                    agent_id TEXT NOT NULL,
                    trace TEXT NOT NULL,
                    timestamp REAL NOT NULL
                );
                CREATE TABLE IF NOT EXISTS memory_lessons (
                    id TEXT PRIMARY KEY,
                    memory_id TEXT NOT NULL,
                    scope TEXT NOT NULL,
                    error_signature TEXT NOT NULL,
                    correction TEXT NOT NULL,
                    severity INTEGER NOT NULL DEFAULT 1,
                    created_at REAL NOT NULL,
                    FOREIGN KEY (memory_id) REFERENCES memories(id)
                );
                CREATE INDEX IF NOT EXISTS idx_memory_lessons_scope
                    ON memory_lessons(scope, severity DESC, created_at DESC);
                CREATE TABLE IF NOT EXISTS nightly_dreams (
                    id TEXT PRIMARY KEY,
                    dream_date TEXT NOT NULL UNIQUE,
                    memory_id TEXT NOT NULL,
                    summary TEXT NOT NULL,
                    positive_count INTEGER NOT NULL DEFAULT 0,
                    negative_count INTEGER NOT NULL DEFAULT 0,
                    reasoning_count INTEGER NOT NULL DEFAULT 0,
                    created_at REAL NOT NULL,
                    FOREIGN KEY (memory_id) REFERENCES memories(id)
                );
                CREATE INDEX IF NOT EXISTS idx_nightly_dreams_date
                    ON nightly_dreams(dream_date DESC);
                CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
                    content, content=memories, content_rowid=rowid
                );
                CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
                    INSERT INTO memories_fts(rowid, content) VALUES (new.rowid, new.content);
                END;
                CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
                    INSERT INTO memories_fts(memories_fts, rowid, content) VALUES('delete', old.rowid, old.content);
                END;
                CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
                    INSERT INTO memories_fts(memories_fts, rowid, content) VALUES('delete', old.rowid, old.content);
                    INSERT INTO memories_fts(rowid, content) VALUES (new.rowid, new.content);
                END;",
            )?;
        }

        let vector = VectorStore::new(db.clone())?;
        let graph_store = GraphStore::new(db.clone())?;
        let graph = Arc::new(Mutex::new(graph_store));
        let hybrid = HybridSearch::new(db.clone(), vector.clone(), graph.clone());
        let memory = AgentMemory::new(db.clone(), vector.clone())?;

        Ok(Self {
            graph,
            vector,
            hybrid,
            memory,
            db,
        })
    }

    // ─── Memory operations ───

    pub fn store_memory(&self, entry: MemoryEntry) -> Result<String> {
        self.memory.store(entry)
    }

    pub fn recall_memory(
        &self,
        query: &str,
        memory_type: Option<MemoryType>,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>> {
        self.memory.recall(query, memory_type, limit)
    }

    // ─── Graph operations ───

    pub fn add_entity(&self, node: GraphNode) -> Result<String> {
        self.graph.lock().unwrap().add_node(node)
    }

    pub fn add_relation(&self, edge: GraphEdge) -> Result<String> {
        self.graph.lock().unwrap().add_edge(edge)
    }

    pub fn graph_query(&self, query: &str, max_hops: usize) -> Result<KnowledgeGraph> {
        let nodes = self.graph.lock().unwrap().search_by_label(query);
        if nodes.is_empty() {
            return Ok(KnowledgeGraph {
                nodes: vec![],
                edges: vec![],
            });
        }
        let seed_id = nodes[0].id.clone();
        Ok(self.graph.lock().unwrap().get_subgraph(&seed_id, max_hops))
    }

    pub fn get_knowledge_graph(&self, filter: &str, depth: usize) -> Result<KnowledgeGraph> {
        let graph = self.graph.lock().unwrap();
        if filter.is_empty() {
            Ok(KnowledgeGraph {
                nodes: graph.all_nodes(100),
                edges: graph.all_edges(200),
            })
        } else {
            Ok(graph.get_subgraph(filter, depth))
        }
    }

    // ─── Hybrid search ───

    pub fn hybrid_search(
        &self,
        query: &str,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<ScoredResult>> {
        self.hybrid.hybrid_search(query, query_embedding, limit)
    }

    pub fn graph_rag(
        &self,
        query: &str,
        query_embedding: &[f32],
        max_hops: usize,
    ) -> Result<GraphRagResult> {
        self.hybrid.graph_rag(query, query_embedding, max_hops)
    }
}
