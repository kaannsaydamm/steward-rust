//! Knowledge-graph REST bridge — read-only HTTP views of the graph RPCs
//! (`GraphQuery` / `GetKnowledgeGraph`) so the Hermes-derived web dashboard
//! can render the Steward knowledge graph (the old web-ui KGViewer surface)
//! without a gRPC client.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::MySteward;

/// GET /api/kg/graph?filter=&depth=
pub async fn graph(
    State(steward): State<MySteward>,
    axum::extract::Query(params): axum::extract::Query<GraphParams>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let depth = params.depth.unwrap_or(2).clamp(0, 6);
    let graph = steward
        .knowledge
        .get_knowledge_graph(&params.filter.unwrap_or_default(), depth as usize)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(json!({
        "nodes": graph.nodes.iter().map(|node| json!({
            "id": node.id,
            "label": node.label,
            "type": node.node_type,
        })).collect::<Vec<_>>(),
        "edges": graph.edges.iter().map(|edge| json!({
            "id": edge.id,
            "source": edge.source_id,
            "target": edge.target_id,
            "relationship": edge.relationship,
            "weight": edge.weight,
        })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub struct GraphParams {
    #[serde(default)]
    filter: Option<String>,
    depth: Option<i32>,
}

#[derive(Deserialize)]
pub struct QueryBody {
    query: String,
    #[serde(default)]
    max_hops: Option<i32>,
    #[serde(default)]
    limit: Option<i32>,
}

/// POST /api/kg/query
pub async fn query(
    State(steward): State<MySteward>,
    Json(body): Json<QueryBody>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if body.query.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "query required".into()));
    }
    let request = steward_core::pb::GraphQueryRequest {
        query: body.query,
        max_hops: body.max_hops.unwrap_or(2),
        limit: body.limit.unwrap_or(50),
    };
    let response = crate::rpc::knowledge::graph_query(
        &steward,
        tonic::Request::new(request),
    )
    .await
    .map_err(|status| (StatusCode::INTERNAL_SERVER_ERROR, status.message().to_string()))?;
    let inner = response.into_inner();
    Ok(Json(json!({
        "nodes": inner.nodes.iter().map(|node| json!({
            "id": node.id,
            "label": node.label,
            "type": node.node_type,
        })).collect::<Vec<_>>(),
        "edges": inner.edges.iter().map(|edge| json!({
            "id": edge.id,
            "source": edge.source_id,
            "target": edge.target_id,
            "relationship": edge.relationship,
            "weight": edge.weight,
        })).collect::<Vec<_>>(),
    })))
}
