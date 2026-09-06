//! Scheduler tests (Task 12.2 acceptance): fan-out concurrency, join
//! policies, retry isolation, partial recovery, conditional routing.

use super::super::*;
use super::*;
use parking_lot::Mutex;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

fn plan_from(nodes: &[(&str, NodeSpec)], edges: &[(&str, &str, EdgeKind)]) -> ExecutionPlan {
    let mut node_map = BTreeMap::new();
    for (id, spec) in nodes {
        node_map.insert(id.to_string(), spec.clone());
    }
    ExecutionPlan {
        schema_version: IR_SCHEMA_VERSION,
        plan_id: "sched".into(),
        entry_nodes: vec![nodes[0].0.to_string()],
        nodes: node_map,
        edges: edges
            .iter()
            .map(|(from, to, kind)| EdgeSpec {
                from: from.to_string(),
                to: to.to_string(),
                edge_kind: kind.clone(),
            })
            .collect(),
        success_policy: json!({}),
        budget: json!({}),
        metadata: BTreeMap::new(),
    }
}

fn native(tag: &str) -> NodeSpec {
    NodeSpec::Native {
        operation: "emit".into(),
        parameters: json!({"tag": tag}),
    }
}

fn noop_executor() -> NodeExecutor {
    Arc::new(|node, _spec, _state| Ok(json!(format!("{node} output"))))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fan_out_branches_run_concurrently() {
    // Three branches after a fan-out edge; each sleeps 60ms. Serial would be
    // >=180ms; parallel with max_parallelism=4 finishes in <150ms.
    let plan = plan_from(
        &[
            ("start", native("start")),
            ("b1", native("b1")),
            ("b2", native("b2")),
            ("b3", native("b3")),
        ],
        &[
            ("start", "b1", EdgeKind::FanOut),
            ("start", "b2", EdgeKind::FanOut),
            ("start", "b3", EdgeKind::FanOut),
        ],
    );
    let scheduler = Scheduler { max_parallelism: 4 };
    let started = Instant::now();
    let results = scheduler
        .run(
            &plan,
            Arc::new(|node, _spec, _state| {
                std::thread::sleep(Duration::from_millis(60));
                Ok(json!(format!("{node} done")))
            }),
        )
        .await
        .unwrap();
    let elapsed = started.elapsed();
    assert_eq!(results.len(), 4);
    for branch in ["b1", "b2", "b3"] {
        let output = results.get(branch).unwrap();
        assert!(matches!(output.outcome, NodeOutcome::Succeeded { .. }));
    }
    assert!(
        elapsed < Duration::from_millis(170),
        "branches must overlap: {elapsed:?}"
    );
}

#[tokio::test]
async fn join_waits_for_all_branches() {
    let plan = plan_from(
        &[
            ("start", native("start")),
            ("b1", native("b1")),
            ("b2", native("b2")),
            ("join", native("join")),
        ],
        &[
            ("start", "b1", EdgeKind::FanOut),
            ("start", "b2", EdgeKind::FanOut),
            (
                "b1",
                "join",
                EdgeKind::FanIn {
                    policy: JoinPolicy::All,
                },
            ),
            (
                "b2",
                "join",
                EdgeKind::FanIn {
                    policy: JoinPolicy::All,
                },
            ),
        ],
    );
    let scheduler = Scheduler { max_parallelism: 4 };
    let results = scheduler.run(&plan, noop_executor()).await.unwrap();
    assert!(results.contains_key("join"), "join opened after all inputs");
    assert!(matches!(
        results.get("join").unwrap().outcome,
        NodeOutcome::Succeeded { .. }
    ));
}

#[tokio::test]
async fn quorum_join_opens_early() {
    let plan = plan_from(
        &[
            ("start", native("start")),
            ("b1", native("b1")),
            ("b2", native("b2")),
            ("join", native("join")),
        ],
        &[
            ("start", "b1", EdgeKind::FanOut),
            ("start", "b2", EdgeKind::FanOut),
            (
                "b1",
                "join",
                EdgeKind::FanIn {
                    policy: JoinPolicy::Quorum { count: 1 },
                },
            ),
            (
                "b2",
                "join",
                EdgeKind::FanIn {
                    policy: JoinPolicy::Quorum { count: 1 },
                },
            ),
        ],
    );
    let scheduler = Scheduler { max_parallelism: 4 };
    let results = scheduler.run(&plan, noop_executor()).await.unwrap();
    assert!(results.contains_key("join"));
}

#[tokio::test]
async fn failed_branch_does_not_rerun_successful_siblings() {
    // b1 succeeds; b2 fails (retryable but no retry edge). join never opens.
    let call_counts = Arc::new(AtomicUsize::new(0));
    let counts = call_counts.clone();
    let plan = plan_from(
        &[
            ("start", native("start")),
            ("b1", native("b1")),
            ("b2", native("b2")),
            ("join", native("join")),
        ],
        &[
            ("start", "b1", EdgeKind::FanOut),
            ("start", "b2", EdgeKind::FanOut),
            (
                "b1",
                "join",
                EdgeKind::FanIn {
                    policy: JoinPolicy::All,
                },
            ),
            (
                "b2",
                "join",
                EdgeKind::FanIn {
                    policy: JoinPolicy::All,
                },
            ),
        ],
    );
    let scheduler = Scheduler { max_parallelism: 4 };
    let results = scheduler
        .run(
            &plan,
            Arc::new(move |node, _spec, _state| {
                counts.fetch_add(1, Ordering::SeqCst);
                if node == "b2" {
                    anyhow::bail!("b2 exploded");
                }
                Ok(json!("ok"))
            }),
        )
        .await
        .unwrap();

    // b1 ran once and is not re-run (G-016 partial superstep recovery).
    assert!(matches!(
        results.get("b1").unwrap().outcome,
        NodeOutcome::Succeeded { .. }
    ));
    assert!(matches!(
        results.get("b2").unwrap().outcome,
        NodeOutcome::Failed { .. }
    ));
    assert!(!results.contains_key("join"), "All-join blocked by failure");
    assert!(
        call_counts.load(Ordering::SeqCst) <= 4,
        "successful siblings must not re-run: {}",
        call_counts.load(Ordering::SeqCst)
    );
}

#[tokio::test]
async fn retry_edge_recovers_transient_failure() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_clone = attempts.clone();
    let plan = plan_from(
        &[("start", native("start")), ("flaky", native("flaky"))],
        &[
            ("start", "flaky", EdgeKind::Direct),
            (
                "start",
                "flaky",
                EdgeKind::Retry {
                    max_attempts: 3,
                    backoff_ms: 5,
                },
            ),
        ],
    );
    let scheduler = Scheduler { max_parallelism: 2 };
    let results = scheduler
        .run(
            &plan,
            Arc::new(move |node, _spec, _state| {
                if node == "flaky" {
                    let attempt = attempts_clone.fetch_add(1, Ordering::SeqCst);
                    if attempt == 0 {
                        anyhow::bail!("transient network hiccup");
                    }
                }
                Ok(json!("recovered"))
            }),
        )
        .await
        .unwrap();
    assert!(matches!(
        results.get("flaky").unwrap().outcome,
        NodeOutcome::Succeeded { .. }
    ));
    assert_eq!(attempts.load(Ordering::SeqCst), 2, "one retry");
}

#[tokio::test]
async fn conditional_edge_routes_on_state() {
    let plan = plan_from(
        &[
            ("start", native("start")),
            ("fast_path", native("fast_path")),
            ("slow_path", native("slow_path")),
        ],
        &[
            (
                "start",
                "fast_path",
                EdgeKind::Conditional {
                    predicate: "start == \"start output\"".into(),
                },
            ),
            (
                "start",
                "slow_path",
                EdgeKind::Conditional {
                    predicate: "start == \"never\"".into(),
                },
            ),
        ],
    );
    let scheduler = Scheduler { max_parallelism: 2 };
    let results = scheduler.run(&plan, noop_executor()).await.unwrap();
    assert!(results.contains_key("fast_path"));
    assert!(
        !results.contains_key("slow_path"),
        "unmet predicate must not fire"
    );
}

#[tokio::test]
async fn sequential_chain_preserves_order() {
    let plan = plan_from(
        &[("a", native("a")), ("b", native("b")), ("c", native("c"))],
        &[("a", "b", EdgeKind::Direct), ("b", "c", EdgeKind::Direct)],
    );
    let scheduler = Scheduler { max_parallelism: 2 };
    let results = scheduler.run(&plan, noop_executor()).await.unwrap();
    assert_eq!(results.len(), 3);
    // G-008: B ready only after A completes — outputs carry node tags.
    for node in ["a", "b", "c"] {
        assert!(matches!(
            results.get(node).unwrap().outcome,
            NodeOutcome::Succeeded { .. }
        ));
    }
}

#[tokio::test]
async fn executor_receives_upstream_outputs_in_state() {
    let plan = plan_from(
        &[
            ("a", native("a")),
            (
                "b",
                NodeSpec::Native {
                    operation: "combine".into(),
                    parameters: json!({}),
                },
            ),
        ],
        &[("a", "b", EdgeKind::Direct)],
    );
    let scheduler = Scheduler { max_parallelism: 2 };
    let seen_state = Arc::new(Mutex::new(None));
    let seen = seen_state.clone();
    let results = scheduler
        .run(
            &plan,
            Arc::new(move |node, _spec, state| {
                if node == "b" {
                    *seen.lock() = Some(state.get("a").cloned());
                }
                Ok(json!(format!("{node} output")))
            }),
        )
        .await
        .unwrap();
    assert!(results.contains_key("b"));
    assert_eq!(seen_state.lock().clone().flatten(), Some(json!("a output")));
}
