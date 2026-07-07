use super::*;

fn setup() -> Connection {
    let connection = Connection::open_in_memory().expect("open database");
    create_schema(&connection).expect("create schema");
    connection
}

#[test]
fn create_rejects_empty_title() {
    let connection = setup();
    let error = create(&connection, "  ", ArtifactKind::Text, "hi", "", "").unwrap_err();
    assert!(error.to_string().contains("title is required"));
}

#[test]
fn create_and_get_round_trip() {
    let connection = setup();
    let artifact = create(
        &connection,
        "Modern SaaS Landing Page",
        ArtifactKind::Code,
        "<html></html>",
        "html",
        "session-1",
    )
    .expect("create artifact");
    let fetched = get(&connection, &artifact.artifact_id)
        .expect("get artifact")
        .expect("artifact exists");
    assert_eq!(fetched.title, "Modern SaaS Landing Page");
    assert_eq!(fetched.kind, ArtifactKind::Code);
    assert_eq!(fetched.language, "html");
    assert_eq!(fetched.session_id, "session-1");
}

#[test]
fn list_filters_by_query_across_title_and_content() {
    let connection = setup();
    create(
        &connection,
        "Poem about nature",
        ArtifactKind::Text,
        "trees and rivers",
        "",
        "",
    )
    .unwrap();
    create(
        &connection,
        "SQL query",
        ArtifactKind::Code,
        "SELECT 1",
        "sql",
        "",
    )
    .unwrap();

    let by_title = list(&connection, "poem").expect("list by title");
    assert_eq!(by_title.len(), 1);
    assert_eq!(by_title[0].title, "Poem about nature");

    let by_content = list(&connection, "rivers").expect("list by content");
    assert_eq!(by_content.len(), 1);

    let all = list(&connection, "").expect("list all");
    assert_eq!(all.len(), 2);
}

#[test]
fn delete_removes_the_row() {
    let connection = setup();
    let artifact = create(&connection, "temp", ArtifactKind::Text, "x", "", "").unwrap();
    assert!(delete(&connection, &artifact.artifact_id).expect("delete"));
    assert!(get(&connection, &artifact.artifact_id)
        .expect("get")
        .is_none());
    assert!(!delete(&connection, &artifact.artifact_id).expect("delete again"));
}

#[test]
fn create_rejects_oversized_content() {
    let connection = setup();
    let huge = "x".repeat(MAX_CONTENT_BYTES + 1);
    let error = create(&connection, "big", ArtifactKind::Text, &huge, "", "").unwrap_err();
    assert!(error.to_string().contains("exceeds"));
}
