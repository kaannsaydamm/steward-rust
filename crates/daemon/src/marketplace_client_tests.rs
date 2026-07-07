use super::{fetch_skill_markdown, search_connectors, search_skills};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};

async fn mock_server(routes: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock marketplace server");
    let address = listener.local_addr().expect("mock marketplace address");
    let handle = tokio::spawn(async move {
        axum::serve(listener, routes).await.expect("serve mock");
    });
    (format!("http://{address}"), handle)
}

#[tokio::test]
async fn search_connectors_parses_smithery_shaped_response() {
    async fn handler(State(body): State<Value>) -> Json<Value> {
        Json(body)
    }
    let body = json!({"servers": [{
        "qualifiedName": "gmail",
        "displayName": "Gmail",
        "description": "Manage Gmail",
        "homepage": "https://smithery.ai/servers/gmail",
        "verified": true,
        "useCount": 42,
        "remote": true,
    }]});
    let (base_url, server) = mock_server(
        Router::new()
            .route("/servers", get(handler))
            .with_state(body),
    )
    .await;

    let entries = search_connectors(
        &reqwest::Client::new(),
        &format!("{base_url}/servers"),
        "gmail",
    )
    .await
    .expect("search connectors");

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].qualified_name, "gmail");
    assert_eq!(entries[0].use_count, 42);
    assert!(entries[0].verified);
    server.abort();
}

#[tokio::test]
async fn search_skills_parses_clawhub_shaped_response() {
    async fn handler(State(body): State<Value>) -> Json<Value> {
        Json(body)
    }
    let body = json!({"items": [{
        "slug": "second-order-thinking",
        "displayName": "Second-Order Thinking",
        "summary": "Trace downstream consequences",
        "topics": ["psychology", "decisions"],
        "stats": {"downloads": 70, "stars": 3},
    }]});
    let (base_url, server) = mock_server(
        Router::new()
            .route("/skills", get(handler))
            .with_state(body),
    )
    .await;

    let entries = search_skills(
        &reqwest::Client::new(),
        &format!("{base_url}/skills"),
        "thinking",
    )
    .await
    .expect("search skills");

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].slug, "second-order-thinking");
    assert_eq!(entries[0].downloads, 70);
    assert_eq!(entries[0].topics, vec!["psychology", "decisions"]);
    server.abort();
}

#[tokio::test]
async fn fetch_skill_markdown_returns_display_name_and_description() {
    async fn handler(State(body): State<Value>) -> Json<Value> {
        Json(body)
    }
    let body = json!({"skill": {
        "displayName": "Second-Order Thinking",
        "description": "---\nname: second-order-thinking\n---\n\n# Second-Order Thinking",
    }});
    let (base_url, server) = mock_server(
        Router::new()
            .route("/skills/second-order-thinking", get(handler))
            .with_state(body),
    )
    .await;

    let (display_name, markdown) = fetch_skill_markdown(
        &reqwest::Client::new(),
        &format!("{base_url}/skills"),
        "second-order-thinking",
    )
    .await
    .expect("fetch skill markdown");

    assert_eq!(display_name, "Second-Order Thinking");
    assert!(markdown.contains("# Second-Order Thinking"));
    server.abort();
}

#[tokio::test]
async fn fetch_skill_markdown_rejects_invalid_slug() {
    let error = fetch_skill_markdown(
        &reqwest::Client::new(),
        "http://127.0.0.1:1/skills",
        "../etc",
    )
    .await
    .expect_err("invalid slug must be rejected");

    assert!(error.to_string().contains("invalid skill slug"));
}
