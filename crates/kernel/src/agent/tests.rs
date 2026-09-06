//! TurnEngine scenario tests (Task 4.2 acceptance): scripted models only,
//! no network. Scenarios: text-only, tool action, multiple actions,
//! budget exhaustion, success-policy termination, empty-response stall,
//! emergency ceiling, model error.

use parking_lot::Mutex;
use crate::action::AgentAction;
use crate::agent::{TurnEngine, TurnOutcome};
use crate::budget::Budget;
use crate::services::{
    KernelMessage, KernelServices, ModelRequest, ModelResponse, ToolOutcome, ToolSchema,
};
use crate::success::{PolicyEvaluator, SuccessCriterion, SuccessPolicy};
use std::sync::Arc;

// ------------------------------------------------------------- test fakes ---

#[derive(Clone)]
struct FakeModel {
    replies: Arc<Mutex<Vec<ModelResponse>>>,
    calls: Arc<Mutex<Vec<ModelRequest>>>,
}

impl FakeModel {
    fn new(replies: Vec<ModelResponse>) -> Self {
        Self { replies: Arc::new(Mutex::new(replies)), calls: Arc::new(Mutex::new(Vec::new())) }
    }

    fn calls_made(&self) -> usize {
        self.calls.lock().len()
    }

    fn last_request(&self) -> ModelRequest {
        self.calls.lock().last().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl crate::services::ModelService for FakeModel {
    async fn complete(&self, request: ModelRequest) -> anyhow::Result<ModelResponse> {
        self.calls.lock().push(request);
        let mut queue = self.replies.lock();
        if queue.is_empty() {
            anyhow::bail!("fake model exhausted");
        }
        Ok(queue.remove(0))
    }
}

#[derive(Default)]
struct RecordingTools {
    executed: Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl crate::services::ToolExecutor for RecordingTools {
    async fn execute(&self, action: &AgentAction) -> anyhow::Result<ToolOutcome> {
        match action {
            AgentAction::Tool { call_id, name, .. } => {
                self.executed.lock().push(name.clone());
                Ok(ToolOutcome {
                    call_id: call_id.clone(),
                    ok: true,
                    observation: format!("{name} result"),
                })
            }
            _ => anyhow::bail!("unexpected action for tools"),
        }
    }
}


fn text_response(text: &str) -> ModelResponse {
    ModelResponse {
        text: text.to_owned(),
        ..Default::default()
    }
}

fn tool_action(name: &str) -> AgentAction {
    AgentAction::Tool {
        call_id: format!("call_{name}"),
        name: name.to_owned(),
        arguments: serde_json::json!({}),
    }
}

fn tool_response(names: &[&str]) -> ModelResponse {
    ModelResponse {
        actions: names.iter().map(|n| tool_action(n)).collect(),
        prompt_tokens: 10,
        completion_tokens: 5,
        ..Default::default()
    }
}

fn services(model: FakeModel, tools: Arc<RecordingTools>) -> KernelServices {
    KernelServices {
        models: Arc::new(model),
        tools,
        success: Arc::new(PolicyEvaluator { check: |c| match c {
            SuccessCriterion::FileExists { path } => Ok(path == "done.txt"),
            _ => Ok(false),
        } }),
        events: Arc::new(crate::services::NullSink),
    }
}

fn history() -> Vec<KernelMessage> {
    vec![KernelMessage { role: "user".into(), content: "go".into(), tool_call_id: None }]
}

// ----------------------------------------------------------------- tests ---

#[tokio::test]
async fn completes_on_final_without_actions() {
    let model = FakeModel::new(vec![ModelResponse {
        actions: vec![AgentAction::Final { text: "answer".into() }],
        ..Default::default()
    }]);
    let tools = Arc::new(RecordingTools::default());
    let outcome = TurnEngine::default()
        .run(&services(model, tools.clone()), history(), vec![], &Budget::default(), &SuccessPolicy::AgentDeclared)
        .await
        .unwrap();
    assert_eq!(outcome, TurnOutcome::Completed { final_text: "answer".into() });
    assert!(tools.executed.lock().is_empty());
}

#[tokio::test]
async fn completes_on_text_only_response() {
    let model = FakeModel::new(vec![text_response("plain answer")]);
    let tools = Arc::new(RecordingTools::default());
    let outcome = TurnEngine::default()
        .run(&services(model, tools), history(), vec![], &Budget::default(), &SuccessPolicy::AgentDeclared)
        .await
        .unwrap();
    assert_eq!(outcome, TurnOutcome::Completed { final_text: "plain answer".into() });
}

#[tokio::test]
async fn tool_action_executes_once_and_enters_history_canonically() {
    let model = FakeModel::new(vec![
        tool_response(&["fs.read"]),
        ModelResponse { actions: vec![AgentAction::Final { text: "done".into() }], ..Default::default() },
    ]);
    let tools = Arc::new(RecordingTools::default());
    let engine = TurnEngine::default();
    // Re-run with history capture via a shared spy model.
    let spy_model = FakeModel::new(vec![
        tool_response(&["fs.read"]),
        ModelResponse { actions: vec![AgentAction::Final { text: "done".into() }], ..Default::default() },
    ]);
    let model_for_assert = spy_model.clone();
    let outcome = engine
        .run(&services(spy_model, tools.clone()), history(), vec![], &Budget::default(), &SuccessPolicy::AgentDeclared)
        .await
        .unwrap();
    assert_eq!(outcome, TurnOutcome::Completed { final_text: "done".into() });
    assert_eq!(*tools.executed.lock(), vec!["fs.read".to_owned()], "single execution");

    let second = model_for_assert.last_request();
    let tool_messages: Vec<_> = second.messages.iter().filter(|m| m.role == "tool").collect();
    assert_eq!(tool_messages.len(), 1, "exactly one canonical tool observation");
    assert_eq!(tool_messages[0].tool_call_id.as_deref(), Some("call_fs.read"));
}

#[tokio::test]
async fn multiple_actions_execute_all() {
    let model = FakeModel::new(vec![
        tool_response(&["fs.read", "fs.search"]),
        ModelResponse { actions: vec![AgentAction::Final { text: "ok".into() }], ..Default::default() },
    ]);
    let tools = Arc::new(RecordingTools::default());
    TurnEngine::default()
        .run(&services(model, tools.clone()), history(), vec![], &Budget::default(), &SuccessPolicy::AgentDeclared)
        .await
        .unwrap();
    assert_eq!(*tools.executed.lock(), vec!["fs.read".to_owned(), "fs.search".to_owned()]);
}

#[tokio::test]
async fn tool_budget_exhausts_before_ceiling() {
    // Every reply wants a tool; max_tool_calls=1 stops after the first.
    let model = FakeModel::new((0..32).map(|_| tool_response(&["fs.read"])).collect());
    let tools = Arc::new(RecordingTools::default());
    let budget = Budget { max_tool_calls: Some(1), ..Default::default() };
    let outcome = TurnEngine { emergency_ceiling: 64 }
        .run(&services(model, tools), history(), vec![], &budget, &SuccessPolicy::AgentDeclared)
        .await
        .unwrap();
    match outcome {
        TurnOutcome::Failed { reason } => {
            assert!(reason.contains("max_tool_calls"), "reason: {reason}");
        }
        other => panic!("expected budget failure, got {other:?}"),
    }
}

#[tokio::test]
async fn model_call_budget_stops_run() {
    // Tool-calling replies never complete naturally, so the run loops and
    // the budget gate stops it before the next generation.
    let model = FakeModel::new((0..8).map(|_| tool_response(&["fs.read"])).collect());
    let tools = Arc::new(RecordingTools::default());
    let budget = Budget { max_model_calls: Some(2), ..Default::default() };
    let outcome = TurnEngine::default()
        .run(&services(model, tools), history(), vec![], &budget, &SuccessPolicy::AgentDeclared)
        .await
        .unwrap();
    assert!(
        matches!(&outcome, TurnOutcome::Failed { reason } if reason.contains("max_model_calls")),
        "got {outcome:?}"
    );
}

#[tokio::test]
async fn success_policy_terminates_when_criteria_hold() {
    // The tool writes done.txt (simulated by checker) — after one tool call
    // the policy is satisfied and the run completes even without Final.
    let model = FakeModel::new(vec![tool_response(&["fs.write"])]);
    let tools = Arc::new(RecordingTools::default());
    let policy = SuccessPolicy::All(vec![SuccessCriterion::FileExists { path: "done.txt".into() }]);
    let outcome = TurnEngine::default()
        .run(&services(model, tools), history(), vec![], &Budget::default(), &policy)
        .await
        .unwrap();
    match outcome {
        TurnOutcome::Completed { final_text } => assert_eq!(final_text, ""),
        other => panic!("expected completion, got {other:?}"),
    }
}

#[tokio::test]
async fn model_error_fails_the_run() {
    struct BrokenModel;
    #[async_trait::async_trait]
    impl crate::services::ModelService for BrokenModel {
        async fn complete(&self, _request: ModelRequest) -> anyhow::Result<ModelResponse> {
            anyhow::bail!("provider down")
        }
    }
    let services = KernelServices {
        models: Arc::new(BrokenModel),
        tools: Arc::new(RecordingTools::default()),
        success: Arc::new(PolicyEvaluator { check: |_| Ok(false) }),
        events: Arc::new(crate::services::NullSink),
    };
    let outcome = TurnEngine::default()
        .run(&services, history(), vec![], &Budget::default(), &SuccessPolicy::AgentDeclared)
        .await
        .unwrap();
    assert!(
        matches!(&outcome, TurnOutcome::Failed { reason } if reason.contains("provider down")),
        "got {outcome:?}"
    );
}

#[tokio::test]
async fn emergency_ceiling_is_configurable_and_distinct() {
    // Tool-only replies with an unsatisfiable policy loop; the ceiling (2)
    // must cut in with CeilingExceeded, not a budget failure.
    let model = FakeModel::new((0..4).map(|_| tool_response(&["fs.read"])).collect());
    let tools = Arc::new(RecordingTools::default());
    let never = SuccessPolicy::All(vec![SuccessCriterion::FileExists { path: "never".into() }]);
    let outcome = TurnEngine { emergency_ceiling: 2 }
        .run(&services(model, tools), history(), vec![], &Budget::default(), &never)
        .await
        .unwrap();
    assert_eq!(outcome, TurnOutcome::CeilingExceeded);
}

#[tokio::test]
async fn tool_definitions_are_passed_to_the_model() {
    let model = FakeModel::new(vec![text_response("hi")]);
    let tools_def = vec![ToolSchema {
        name: "fs.read".into(),
        description: "read".into(),
        input_schema: serde_json::json!({"type": "object"}),
    }];
    let spy = model.clone();
    TurnEngine::default()
        .run(
            &services(model, Arc::new(RecordingTools::default())),
            history(),
            tools_def.clone(),
            &Budget::default(),
            &SuccessPolicy::AgentDeclared,
        )
        .await
        .unwrap();
    assert_eq!(spy.last_request().tools, tools_def);
}
