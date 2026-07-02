use super::*;
use steward_core::pb::{WorkflowConnection, WorkflowNode};

fn graph() -> WorkflowDefinition {
    WorkflowDefinition {
        name: "Ship".to_owned(),
        nodes: vec![node("plan"), node("build"), node("review")],
        connections: vec![edge("plan", "build"), edge("build", "review")],
        ..WorkflowDefinition::default()
    }
}

#[test]
fn validates_orders_and_persists_a_dag() {
    let db = Connection::open_in_memory().expect("memory db");
    create_schema(&db).expect("schema");
    let saved = save(&db, graph()).expect("save graph");

    assert_eq!(
        validate_and_order(&saved).unwrap(),
        ["plan", "build", "review"]
    );
    assert_eq!(list(&db).unwrap(), std::slice::from_ref(&saved));
    assert_eq!(get(&db, &saved.definition_id).unwrap(), Some(saved));
}

#[test]
fn rejects_cycles_and_unknown_nodes() {
    let mut cyclic = graph();
    cyclic.connections.push(edge("review", "plan"));
    assert!(validate_and_order(&cyclic)
        .unwrap_err()
        .to_string()
        .contains("cycle"));

    let mut unknown = graph();
    unknown.connections.push(edge("plan", "missing"));
    assert!(validate_and_order(&unknown)
        .unwrap_err()
        .to_string()
        .contains("unknown node"));
}

fn node(id: &str) -> WorkflowNode {
    WorkflowNode {
        node_id: id.to_owned(),
        title: id.to_owned(),
        instruction: format!("Complete {id}"),
        agent_id: "operator".to_owned(),
        allow_tools: true,
        position_x: 0,
        position_y: 0,
    }
}

fn edge(source: &str, target: &str) -> WorkflowConnection {
    WorkflowConnection {
        source_node_id: source.to_owned(),
        target_node_id: target.to_owned(),
    }
}
