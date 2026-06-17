use steward_core::pb::{AgentInfo, WorkflowEvent, WorkflowStatus};

#[derive(Clone)]
pub struct WorkflowState {
    pub status: WorkflowStatus,
    pub events: Vec<WorkflowEvent>,
    pub cancelled: bool,
    pub approved: bool,
    pub mode: i32,
}

#[derive(Clone)]
pub struct InternalAgent {
    pub agent_id: String,
    pub name: String,
    pub role: String,
    pub status: String,
    pub current_task: String,
    pub progress: f32,
}

impl InternalAgent {
    pub fn to_proto(&self) -> AgentInfo {
        AgentInfo {
            agent_id: self.agent_id.clone(),
            name: self.name.clone(),
            role: self.role.clone(),
            status: self.status.clone(),
            current_task: self.current_task.clone(),
            progress: self.progress,
        }
    }
}

pub fn default_agents() -> Vec<InternalAgent> {
    vec![
        agent("architect", "Architect Agent", "System Designer"),
        agent("researcher", "Research Agent", "Information Gatherer"),
        agent("coder", "Coder Agent", "Implementation"),
        agent("reviewer", "Reviewer Agent", "Code Review"),
    ]
}

pub fn new_workflow_state(workflow_id: &str, title: &str) -> WorkflowState {
    WorkflowState {
        status: WorkflowStatus {
            workflow_id: workflow_id.to_owned(),
            title: title.to_owned(),
            phase: 0,
            mode: 0,
            overall_progress: 0.0,
            current_agent: String::new(),
            status_message: "Initializing".to_owned(),
            recent_events: Vec::new(),
            requires_approval: false,
            pending_approval: None,
        },
        events: Vec::new(),
        cancelled: false,
        approved: false,
        mode: 0,
    }
}

pub const WORKFLOW_PHASES: [(i32, &str, &str); 10] = [
    (1, "Discovery", "architect"),
    (2, "Context", "researcher"),
    (3, "Research", "researcher"),
    (4, "Intake", "architect"),
    (5, "Orchestrate", "architect"),
    (6, "Plan", "architect"),
    (7, "Awaiting Approval", ""),
    (8, "Executing", "coder"),
    (9, "Validating", "reviewer"),
    (10, "Completed", ""),
];

pub fn push_recent_event(state: &mut WorkflowState, event: WorkflowEvent) {
    state.events.push(event.clone());
    state.status.recent_events.push(event);
    if state.status.recent_events.len() > 20 {
        state.status.recent_events.remove(0);
    }
}

fn agent(agent_id: &str, name: &str, role: &str) -> InternalAgent {
    InternalAgent {
        agent_id: agent_id.to_owned(),
        name: name.to_owned(),
        role: role.to_owned(),
        status: "idle".to_owned(),
        current_task: String::new(),
        progress: 0.0,
    }
}
