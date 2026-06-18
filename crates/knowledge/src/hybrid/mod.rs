use crate::graph::{GraphStore, KnowledgeGraph};
use crate::vector::VectorStore;
use anyhow::Result;
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct ScoredResult {
    pub id: String,
    pub content: String,
    pub score: f64,
    pub sources: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct GraphRagResult {
    pub subgraph: KnowledgeGraph,
    pub context_text: String,
}

const RRF_K: f64 = 60.0;

pub struct HybridSearch {
    db: Arc<Mutex<Connection>>,
    vector: VectorStore,
    graph: Arc<Mutex<GraphStore>>,
}

impl HybridSearch {
    pub fn new(
        db: Arc<Mutex<Connection>>,
        vector: VectorStore,
        graph: Arc<Mutex<GraphStore>>,
    ) -> Self {
        Self { db, vector, graph }
    }

    /// Reciprocal Rank Fusion combining BM25 + vector + graph
    pub fn hybrid_search(
        &self,
        query: &str,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<ScoredResult>> {
        let mut fusion_map: HashMap<String, ScoredResult> = HashMap::new();

        // 1. BM25 via FTS5
        if let Ok(bm25_results) = self.bm25_search(query, limit * 2) {
            for (rank, result) in bm25_results.iter().enumerate() {
                let rrf_score = 1.0 / (RRF_K + rank as f64);
                fusion_map
                    .entry(result.id.clone())
                    .and_modify(|e| {
                        e.score += rrf_score;
                        e.sources.push("bm25".to_string());
                    })
                    .or_insert(ScoredResult {
                        id: result.id.clone(),
                        content: result.content.clone(),
                        score: rrf_score,
                        sources: vec!["bm25".to_string()],
                    });
            }
        }

        // 2. Vector search
        if query_embedding.len() >= 128 {
            if let Ok(vector_results) = self.vector.search_memories(query_embedding, limit * 2) {
                for (rank, result) in vector_results.iter().enumerate() {
                    let rrf_score = 1.0 / (RRF_K + rank as f64);
                    fusion_map
                        .entry(result.id.clone())
                        .and_modify(|e| {
                            e.score += rrf_score;
                            e.sources.push("vector".to_string());
                        })
                        .or_insert(ScoredResult {
                            id: result.id.clone(),
                            content: String::new(),
                            score: rrf_score,
                            sources: vec!["vector".to_string()],
                        });
                }
            }
        }

        // 3. Graph search
        let graph = self.graph.lock().unwrap();
        let matching_nodes = graph.search_by_label(query);
        for (rank, node) in matching_nodes.iter().enumerate() {
            let rrf_score = 1.0 / (RRF_K + rank as f64);
            fusion_map
                .entry(node.id.clone())
                .and_modify(|e| {
                    e.score += rrf_score;
                    e.sources.push("graph".to_string());
                })
                .or_insert(ScoredResult {
                    id: node.id.clone(),
                    content: format!("{}: {}", node.node_type, node.label),
                    score: rrf_score,
                    sources: vec!["graph".to_string()],
                });

            let neighbor_score = rrf_score * 0.5;
            let neighbors = graph.traverse(&node.id, 2);
            for (neighbor, _edge) in neighbors {
                fusion_map
                    .entry(neighbor.id.clone())
                    .and_modify(|e| {
                        e.score += neighbor_score;
                        e.sources.push("graph".to_string());
                    })
                    .or_insert(ScoredResult {
                        id: neighbor.id.clone(),
                        content: format!("{}: {}", neighbor.node_type, neighbor.label),
                        score: neighbor_score,
                        sources: vec!["graph".to_string()],
                    });
            }
        }
        drop(graph);

        let mut results: Vec<ScoredResult> = fusion_map.into_values().collect();
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        Ok(results)
    }

    /// GraphRAG: seed entities → graph expansion → LLM context
    pub fn graph_rag(
        &self,
        query: &str,
        query_embedding: &[f32],
        max_hops: usize,
    ) -> Result<GraphRagResult> {
        let seed_ids = if query_embedding.len() >= 128 {
            self.vector
                .search_entities(query_embedding, 5)?
                .into_iter()
                .map(|r| r.id)
                .collect::<Vec<_>>()
        } else {
            let graph = self.graph.lock().unwrap();
            graph
                .search_by_label(query)
                .into_iter()
                .take(5)
                .map(|n| n.id.clone())
                .collect::<Vec<_>>()
        };

        if seed_ids.is_empty() {
            return Ok(GraphRagResult {
                subgraph: KnowledgeGraph {
                    nodes: vec![],
                    edges: vec![],
                },
                context_text: String::new(),
            });
        }

        let graph = self.graph.lock().unwrap();
        let mut all_nodes = Vec::new();
        let mut all_edges = Vec::new();
        let mut seen_nodes = std::collections::HashSet::new();
        let mut seen_edges = std::collections::HashSet::new();

        for seed_id in &seed_ids {
            let sub = graph.get_subgraph(seed_id, max_hops);
            for node in sub.nodes {
                if seen_nodes.insert(node.id.clone()) {
                    all_nodes.push(node);
                }
            }
            for edge in sub.edges {
                if seen_edges.insert(edge.id.clone()) {
                    all_edges.push(edge);
                }
            }
        }
        drop(graph);

        let mut context_lines = Vec::new();
        context_lines.push("=== Knowledge Graph Context ===".to_string());
        for node in &all_nodes {
            let props: Vec<String> = node
                .properties
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect();
            context_lines.push(format!(
                "[{}] {} — {}",
                node.node_type,
                node.label,
                props.join(", ")
            ));
        }
        context_lines.push("--- Relationships ---".to_string());
        for edge in &all_edges {
            context_lines.push(format!(
                "{} --[{}]--> {}",
                edge.source_id, edge.relationship, edge.target_id
            ));
        }

        Ok(GraphRagResult {
            subgraph: KnowledgeGraph {
                nodes: all_nodes,
                edges: all_edges,
            },
            context_text: context_lines.join("\n"),
        })
    }

    fn bm25_search(&self, query: &str, limit: usize) -> Result<Vec<ScoredResult>> {
        let sanitized: String = query
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect();

        if sanitized.trim().is_empty() {
            return Ok(vec![]);
        }

        let fts_query = sanitized
            .split_whitespace()
            .map(|w| format!("{}*", w))
            .collect::<Vec<_>>()
            .join(" AND ");

        let sql = "SELECT m.id, m.content, rank
             FROM memories_fts
             JOIN memories m ON memories_fts.rowid = m.rowid
             WHERE memories_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2";

        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(sql)?;
        let results = stmt
            .query_map(rusqlite::params![fts_query, limit as i32], |row| {
                Ok(ScoredResult {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    score: row.get::<_, f64>(2).unwrap_or(0.0),
                    sources: vec!["bm25".to_string()],
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }
}
