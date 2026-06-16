use anyhow::Result;
use petgraph::stable_graph::{DefaultIx, StableDiGraph};
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub node_type: String,
    pub properties: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub relationship: String,
    pub properties: HashMap<String, String>,
    pub weight: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KnowledgeGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

struct NodeData {
    node: GraphNode,
}

struct EdgeData {
    edge: GraphEdge,
}

pub struct GraphStore {
    db: Arc<Mutex<Connection>>,
    graph: StableDiGraph<NodeData, EdgeData, DefaultIx>,
    node_indices: HashMap<String, petgraph::stable_graph::NodeIndex<DefaultIx>>,
}

impl GraphStore {
    pub fn new(db: Arc<Mutex<Connection>>) -> Result<Self> {
        let mut store = Self {
            db,
            graph: StableDiGraph::new(),
            node_indices: HashMap::new(),
        };
        store.load_from_db()?;
        Ok(store)
    }

    pub fn add_node(&mut self, node: GraphNode) -> Result<String> {
        let id = if node.id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            node.id.clone()
        };

        let mut node = node;
        node.id = id.clone();

        let idx = self.graph.add_node(NodeData { node: node.clone() });
        self.node_indices.insert(id.clone(), idx);

        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT OR REPLACE INTO kg_nodes (id, label, node_type, properties) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                node.id,
                node.label,
                node.node_type,
                serde_json::to_string(&node.properties)?,
            ],
        )?;

        Ok(id)
    }

    pub fn add_edge(&mut self, edge: GraphEdge) -> Result<String> {
        let id = if edge.id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            edge.id.clone()
        };

        let mut edge = edge;
        edge.id = id.clone();

        let source_idx = self.node_indices.get(&edge.source_id).ok_or_else(|| {
            anyhow::anyhow!("Source node '{}' not found", edge.source_id)
        })?;
        let target_idx = self.node_indices.get(&edge.target_id).ok_or_else(|| {
            anyhow::anyhow!("Target node '{}' not found", edge.target_id)
        })?;

        self.graph.add_edge(
            *source_idx,
            *target_idx,
            EdgeData { edge: edge.clone() },
        );

        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT OR REPLACE INTO kg_edges (id, source_id, target_id, relationship, properties, weight) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                edge.id,
                edge.source_id,
                edge.target_id,
                edge.relationship,
                serde_json::to_string(&edge.properties)?,
                edge.weight,
            ],
        )?;

        Ok(id)
    }

    pub fn get_node(&self, id: &str) -> Option<GraphNode> {
        self.node_indices
            .get(id)
            .map(|idx| self.graph[*idx].node.clone())
    }

    /// Traverse from start_id up to max_hops depth. Returns (node, edge) pairs.
    pub fn traverse(&self, start_id: &str, max_hops: usize) -> Vec<(GraphNode, GraphEdge)> {
        let start_idx = match self.node_indices.get(start_id) {
            Some(idx) => *idx,
            None => return vec![],
        };

        let mut results = Vec::new();
        let mut visited = HashMap::new();
        visited.insert(start_idx, 0);

        let mut queue = vec![(start_idx, 0)];
        while let Some((current, depth)) = queue.pop() {
            if depth >= max_hops {
                continue;
            }

            for edge_ref in self.graph.edges_directed(current, Direction::Outgoing) {
                let target = edge_ref.target();
                let next_depth = depth + 1;
                if !visited.contains_key(&target) || visited[&target] > next_depth {
                    visited.insert(target, next_depth);
                    results.push((
                        self.graph[target].node.clone(),
                        self.graph[edge_ref.id()].edge.clone(),
                    ));
                    queue.push((target, next_depth));
                }
            }
        }

        results
    }

    /// Get subgraph from seed node with given depth
    pub fn get_subgraph(&self, seed_id: &str, depth: usize) -> KnowledgeGraph {
        let mut nodes = vec![];
        let mut edges = vec![];
        let mut seen_nodes = std::collections::HashSet::new();
        let mut seen_edges = std::collections::HashSet::new();

        let start_idx = match self.node_indices.get(seed_id) {
            Some(idx) => *idx,
            None => return KnowledgeGraph { nodes, edges },
        };

        let mut queue = std::collections::VecDeque::new();
        queue.push_back((start_idx, 0));
        seen_nodes.insert(start_idx);

        while let Some((current, d)) = queue.pop_front() {
            if let Some(node_data) = self.graph.node_weight(current) {
                nodes.push(node_data.node.clone());
            }

            if d >= depth {
                continue;
            }

            for edge_ref in self.graph.edges(current) {
                let edge_data = &self.graph[edge_ref.id()];
                if seen_edges.insert(edge_ref.id()) {
                    edges.push(edge_data.edge.clone());
                }

                let neighbor = if edge_ref.source() == current {
                    edge_ref.target()
                } else {
                    edge_ref.source()
                };

                if seen_nodes.insert(neighbor) {
                    queue.push_back((neighbor, d + 1));
                }
            }
        }

        KnowledgeGraph { nodes, edges }
    }

    /// Search nodes by label (case-insensitive contains match)
    pub fn search_by_label(&self, pattern: &str) -> Vec<GraphNode> {
        let lower = pattern.to_lowercase();
        self.graph
            .node_weights()
            .filter(|nd| nd.node.label.to_lowercase().contains(&lower))
            .take(20)
            .map(|nd| nd.node.clone())
            .collect()
    }

    /// Get all nodes (capped)
    pub fn all_nodes(&self, limit: usize) -> Vec<GraphNode> {
        self.graph
            .node_weights()
            .take(limit)
            .map(|nd| nd.node.clone())
            .collect()
    }

    /// Get all edges (capped)
    pub fn all_edges(&self, limit: usize) -> Vec<GraphEdge> {
        self.graph
            .edge_weights()
            .take(limit)
            .map(|ed| ed.edge.clone())
            .collect()
    }

    fn load_from_db(&mut self) -> Result<()> {
        let db = self.db.lock().unwrap();

        let mut stmt = db.prepare("SELECT id, label, node_type, properties FROM kg_nodes")?;
        let node_rows = stmt.query_map([], |row| {
            let props_str: String = row.get(3)?;
            let properties: HashMap<String, String> =
                serde_json::from_str(&props_str).unwrap_or_default();
            Ok(GraphNode {
                id: row.get(0)?,
                label: row.get(1)?,
                node_type: row.get(2)?,
                properties,
            })
        })?;

        for node_result in node_rows {
            let node = node_result?;
            let idx = self.graph.add_node(NodeData { node: node.clone() });
            self.node_indices.insert(node.id.clone(), idx);
        }

        let mut stmt = db.prepare(
            "SELECT id, source_id, target_id, relationship, properties, weight FROM kg_edges",
        )?;
        let edge_rows = stmt.query_map([], |row| {
            let props_str: String = row.get(4)?;
            let properties: HashMap<String, String> =
                serde_json::from_str(&props_str).unwrap_or_default();
            Ok(GraphEdge {
                id: row.get(0)?,
                source_id: row.get(1)?,
                target_id: row.get(2)?,
                relationship: row.get(3)?,
                properties,
                weight: row.get(5)?,
            })
        })?;

        for edge_result in edge_rows {
            let edge = edge_result?;
            if let (Some(src), Some(tgt)) = (
                self.node_indices.get(&edge.source_id).cloned(),
                self.node_indices.get(&edge.target_id).cloned(),
            ) {
                self.graph.add_edge(src, tgt, EdgeData {
                    edge: edge.clone(),
                });
            }
        }

        Ok(())
    }
}
