use crate::MySteward;
use steward_core::pb::*;
use steward_knowledge::MemoryType as KMemType;
use tonic::{Request, Response, Status};

pub async fn store_memory(
    steward: &MySteward,
    request: Request<StoreMemoryRequest>,
) -> Result<Response<StoreMemoryResponse>, Status> {
    let entry = request
        .into_inner()
        .entry
        .ok_or_else(|| Status::invalid_argument("missing entry"))?;
    let km_entry = steward_knowledge::MemoryEntry {
        id: entry.id,
        memory_type: memory_type_from_code(entry.memory_type),
        content: entry.content,
        metadata: entry.metadata.into_iter().collect(),
        entities: entry.entities,
        timestamp: entry.timestamp,
    };
    let id = steward
        .knowledge
        .store_memory(km_entry)
        .map_err(|error| Status::internal(error.to_string()))?;
    Ok(Response::new(StoreMemoryResponse { id, stored: true }))
}

pub async fn recall_memory(
    steward: &MySteward,
    request: Request<RecallMemoryRequest>,
) -> Result<Response<RecallMemoryResponse>, Status> {
    let req = request.into_inner();
    let memory_type = optional_memory_type(req.memory_type);
    let entries = steward
        .knowledge
        .recall_memory(&req.query, memory_type, req.limit as usize)
        .map_err(|error| Status::internal(error.to_string()))?;
    let entries = entries
        .into_iter()
        .map(|entry| MemoryEntry {
            id: entry.id,
            memory_type: memory_type_code(entry.memory_type),
            content: entry.content,
            metadata: entry.metadata.into_iter().collect(),
            entities: entry.entities,
            timestamp: entry.timestamp,
            embedding: Vec::new(),
        })
        .collect();
    Ok(Response::new(RecallMemoryResponse { entries }))
}

pub async fn graph_query(
    steward: &MySteward,
    request: Request<GraphQueryRequest>,
) -> Result<Response<GraphQueryResponse>, Status> {
    let req = request.into_inner();
    let graph = steward
        .knowledge
        .graph_query(&req.query, req.max_hops as usize)
        .map_err(|error| Status::internal(error.to_string()))?;
    let nodes = graph.nodes.into_iter().map(graph_node).collect::<Vec<_>>();
    let edges = graph.edges.into_iter().map(graph_edge).collect::<Vec<_>>();
    let summary = format!("Found {} nodes and {} edges", nodes.len(), edges.len());
    Ok(Response::new(GraphQueryResponse {
        nodes,
        edges,
        summary,
    }))
}

pub async fn get_knowledge_graph(
    steward: &MySteward,
    request: Request<GetKnowledgeGraphRequest>,
) -> Result<Response<GetKnowledgeGraphResponse>, Status> {
    let req = request.into_inner();
    let graph = steward
        .knowledge
        .get_knowledge_graph(&req.entity_filter, req.depth as usize)
        .map_err(|error| Status::internal(error.to_string()))?;
    Ok(Response::new(GetKnowledgeGraphResponse {
        nodes: graph.nodes.into_iter().map(graph_node).collect(),
        edges: graph.edges.into_iter().map(graph_edge).collect(),
    }))
}

fn memory_type_from_code(code: i32) -> KMemType {
    match code {
        1 => KMemType::LongTerm,
        2 => KMemType::Reasoning,
        3 => KMemType::Negative,
        4 => KMemType::Dream,
        _ => KMemType::ShortTerm,
    }
}

fn optional_memory_type(code: i32) -> Option<KMemType> {
    match code {
        1 => Some(KMemType::LongTerm),
        2 => Some(KMemType::Reasoning),
        3 => Some(KMemType::Negative),
        4 => Some(KMemType::Dream),
        _ => None,
    }
}

fn memory_type_code(memory_type: KMemType) -> i32 {
    match memory_type {
        KMemType::LongTerm => 1,
        KMemType::Reasoning => 2,
        KMemType::Negative => 3,
        KMemType::Dream => 4,
        KMemType::ShortTerm => 0,
    }
}

fn graph_node(node: steward_knowledge::GraphNode) -> GraphNode {
    GraphNode {
        id: node.id,
        label: node.label,
        node_type: node.node_type,
        properties: node.properties.into_iter().collect(),
        embedding: Vec::new(),
    }
}

fn graph_edge(edge: steward_knowledge::GraphEdge) -> GraphEdge {
    GraphEdge {
        id: edge.id,
        source_id: edge.source_id,
        target_id: edge.target_id,
        relationship: edge.relationship,
        properties: edge.properties.into_iter().collect(),
        weight: edge.weight,
    }
}
