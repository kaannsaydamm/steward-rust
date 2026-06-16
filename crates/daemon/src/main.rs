use anyhow::Result;
use log::info;
use rusqlite::Connection;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use steward_core::pb::*;
use steward_core::pb::steward_service_server::{StewardService, StewardServiceServer};
use steward_knowledge::{KnowledgeEngine, MemoryType as KMemType};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status};
use tonic_web::GrpcWebLayer;
use tower_http::cors::CorsLayer;
use wasmtime::{Engine, Instance, Module, Store};
use futures_util::Stream;

// ─── Stream type aliases ───
type ResponseStream = Pin<Box<dyn Stream<Item = Result<WorkflowEvent, Status>> + Send>>;
type LogStream = Pin<Box<dyn Stream<Item = Result<AgentLogEntry, Status>> + Send>>;

// ─── In-memory workflow state ───
struct WorkflowState {
    status: WorkflowStatus,
    events: Vec<WorkflowEvent>,
    cancelled: bool,
    approved: bool,
    mode: i32,
}

// ─── Agent info struct for internal use ───
#[derive(Clone)]
struct InternalAgent {
    agent_id: String,
    name: String,
    role: String,
    status: String,
    current_task: String,
    progress: f32,
}

// ─── Steward service ───
#[derive(Clone)]
pub struct MySteward {
    db: Arc<Mutex<Connection>>,
    wasm_engine: Engine,
    knowledge: Arc<KnowledgeEngine>,
    workflows: Arc<tokio::sync::Mutex<HashMap<String, WorkflowState>>>,
    agents: Arc<tokio::sync::Mutex<Vec<InternalAgent>>>,
    agent_logs: Arc<tokio::sync::Mutex<HashMap<String, Vec<AgentLogEntry>>>>,
}

// Manual Clone implementation since KnowledgeEngine isn't Clone
// All fields are either Clone, Arc, or Mutex

impl MySteward {
    pub fn new(db_path: &str) -> Result<Self> {
        // Init sqlite-vec auto extension
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }

        let db = Connection::open(db_path)?;

        // Create tasks table
        db.execute(
            "CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY,
                task TEXT NOT NULL
            )",
            [],
        )?;

        let knowledge = KnowledgeEngine::new(db)?;

        // Open a second connection for direct operations
        let direct_db = Connection::open(db_path)?;

        let wasm_engine = Engine::default();

        Ok(Self {
            db: Arc::new(Mutex::new(direct_db)),
            wasm_engine,
            knowledge: Arc::new(knowledge),
            workflows: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            agents: Arc::new(tokio::sync::Mutex::new(vec![
                InternalAgent {
                    agent_id: "architect".into(),
                    name: "Architect Agent".into(),
                    role: "System Designer".into(),
                    status: "idle".into(),
                    current_task: String::new(),
                    progress: 0.0,
                },
                InternalAgent {
                    agent_id: "researcher".into(),
                    name: "Research Agent".into(),
                    role: "Information Gatherer".into(),
                    status: "idle".into(),
                    current_task: String::new(),
                    progress: 0.0,
                },
                InternalAgent {
                    agent_id: "coder".into(),
                    name: "Coder Agent".into(),
                    role: "Implementation".into(),
                    status: "idle".into(),
                    current_task: String::new(),
                    progress: 0.0,
                },
                InternalAgent {
                    agent_id: "reviewer".into(),
                    name: "Reviewer Agent".into(),
                    role: "Code Review".into(),
                    status: "idle".into(),
                    current_task: String::new(),
                    progress: 0.0,
                },
            ])),
            agent_logs: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        })
    }

    fn add_agent_log(&self, workflow_id: &str, agent_id: &str, level: &str, message: &str, detail: &str) {
        if let Ok(mut logs) = self.agent_logs.try_lock() {
            let key = format!("{}/{}", workflow_id, agent_id);
            logs.entry(key).or_insert_with(Vec::new).push(AgentLogEntry {
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs_f64(),
                level: level.to_string(),
                message: message.to_string(),
                detail: detail.to_string(),
            });
        }
    }
}

#[tonic::async_trait]
impl StewardService for MySteward {
    // ─── Ping ───
    async fn ping(
        &self,
        _request: Request<PingRequest>,
    ) -> Result<Response<PingResponse>, Status> {
        Ok(Response::new(PingResponse {
            status: "OK".into(),
        }))
    }

    // ─── RunPlugin ───
    async fn run_plugin(
        &self,
        request: Request<RunPluginRequest>,
    ) -> Result<Response<RunPluginResponse>, Status> {
        let req = request.into_inner();
        let wasm_bytes = req.wasm_binary;

        if wasm_bytes.is_empty() {
            return Err(Status::invalid_argument("WASM binary is empty"));
        }

        let module = Module::from_binary(&self.wasm_engine, &wasm_bytes)
            .map_err(|e| Status::internal(format!("Failed to compile WASM: {}", e)))?;

        let mut store = Store::new(&self.wasm_engine, ());
        let instance = Instance::new(&mut store, &module, &[])
            .map_err(|e| Status::internal(format!("Failed to instantiate WASM: {}", e)))?;

        let run_func = instance
            .get_typed_func::<(), i32>(&mut store, "run")
            .map_err(|e| Status::internal(format!("WASM missing 'run' function: {}", e)))?;

        let result = run_func
            .call(&mut store, ())
            .map_err(|e| Status::internal(format!("Failed to execute 'run': {}", e)))?;

        Ok(Response::new(RunPluginResponse {
            output: format!("Plugin executed successfully with result: {}", result),
        }))
    }

    // ─── ExecuteTask ───
    async fn execute_task(
        &self,
        request: Request<ExecuteTaskRequest>,
    ) -> Result<Response<ExecuteTaskResponse>, Status> {
        let req = request.into_inner();
        let db = self.db.lock().map_err(|_| Status::internal("Database lock failed"))?;
        db.execute("INSERT INTO tasks (task) VALUES (?1)", [&req.task])
            .map_err(|e| Status::internal(format!("Failed to save task: {}", e)))?;
        info!("Task executed and saved: {}", req.task);
        Ok(Response::new(ExecuteTaskResponse {
            status: "Task stored in SQLite successfully".into(),
        }))
    }

    // ─── StoreMemory ───
    async fn store_memory(
        &self,
        request: Request<StoreMemoryRequest>,
    ) -> Result<Response<StoreMemoryResponse>, Status> {
        let req = request.into_inner();
        let entry = req.entry.ok_or_else(|| Status::invalid_argument("missing entry"))?;

        let km_entry = steward_knowledge::MemoryEntry {
            id: entry.id.clone(),
            memory_type: match entry.memory_type {
                1 => KMemType::LongTerm,
                2 => KMemType::Reasoning,
                _ => KMemType::ShortTerm,
            },
            content: entry.content,
            metadata: entry.metadata.into_iter().collect(),
            entities: entry.entities,
            timestamp: entry.timestamp,
        };

        let id = self.knowledge
            .store_memory(km_entry)
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(StoreMemoryResponse {
            id,
            stored: true,
        }))
    }

    // ─── RecallMemory ───
    async fn recall_memory(
        &self,
        request: Request<RecallMemoryRequest>,
    ) -> Result<Response<RecallMemoryResponse>, Status> {
        let req = request.into_inner();
        let mem_type = match req.memory_type {
            1 => Some(KMemType::LongTerm),
            2 => Some(KMemType::Reasoning),
            _ => None,
        };

        let entries = self.knowledge
            .recall_memory(&req.query, mem_type, req.limit as usize)
            .map_err(|e| Status::internal(e.to_string()))?;

        let proto_entries: Vec<MemoryEntry> = entries
            .into_iter()
            .map(|e| MemoryEntry {
                id: e.id,
                memory_type: match e.memory_type {
                    KMemType::LongTerm => 1,
                    KMemType::Reasoning => 2,
                    _ => 0,
                },
                content: e.content,
                metadata: e.metadata.into_iter().collect(),
                entities: e.entities,
                timestamp: e.timestamp,
                embedding: vec![],
            })
            .collect();

        Ok(Response::new(RecallMemoryResponse {
            entries: proto_entries,
        }))
    }

    // ─── GraphQuery ───
    async fn graph_query(
        &self,
        request: Request<GraphQueryRequest>,
    ) -> Result<Response<GraphQueryResponse>, Status> {
        let req = request.into_inner();
        let kg = self.knowledge
            .graph_query(&req.query, req.max_hops as usize)
            .map_err(|e| Status::internal(e.to_string()))?;

        let nodes: Vec<GraphNode> = kg
            .nodes
            .into_iter()
            .map(|n| GraphNode {
                id: n.id,
                label: n.label,
                node_type: n.node_type,
                properties: n.properties.into_iter().collect(),
                embedding: vec![],
            })
            .collect();

        let edges: Vec<GraphEdge> = kg
            .edges
            .into_iter()
            .map(|e| GraphEdge {
                id: e.id,
                source_id: e.source_id,
                target_id: e.target_id,
                relationship: e.relationship,
                properties: e.properties.into_iter().collect(),
                weight: e.weight,
            })
            .collect();

        let summary = format!("Found {} nodes and {} edges", nodes.len(), edges.len());

        Ok(Response::new(GraphQueryResponse {
            nodes,
            edges,
            summary,
        }))
    }

    // ─── GetKnowledgeGraph ───
    async fn get_knowledge_graph(
        &self,
        request: Request<GetKnowledgeGraphRequest>,
    ) -> Result<Response<GetKnowledgeGraphResponse>, Status> {
        let req = request.into_inner();
        let kg = self.knowledge
            .get_knowledge_graph(&req.entity_filter, req.depth as usize)
            .map_err(|e| Status::internal(e.to_string()))?;

        let nodes: Vec<GraphNode> = kg
            .nodes
            .into_iter()
            .map(|n| GraphNode {
                id: n.id,
                label: n.label,
                node_type: n.node_type,
                properties: n.properties.into_iter().collect(),
                embedding: vec![],
            })
            .collect();

        let edges: Vec<GraphEdge> = kg
            .edges
            .into_iter()
            .map(|e| GraphEdge {
                id: e.id,
                source_id: e.source_id,
                target_id: e.target_id,
                relationship: e.relationship,
                properties: e.properties.into_iter().collect(),
                weight: e.weight,
            })
            .collect();

        Ok(Response::new(GetKnowledgeGraphResponse { nodes, edges }))
    }

    // ─── StartWorkflow (server-streaming) ───
    type StartWorkflowStream = ResponseStream;

    async fn start_workflow(
        &self,
        request: Request<StartWorkflowRequest>,
    ) -> Result<Response<Self::StartWorkflowStream>, Status> {
        let req = request.into_inner();
        let workflow_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::channel(128);

        let workflows = self.workflows.clone();
        let agent_logs = self.agent_logs.clone();
        let title = req.title.clone();
        let description = req.description.clone();

        // Initialize workflow state
        {
            let mut wf = workflows.lock().await;
            wf.insert(
                workflow_id.clone(),
                WorkflowState {
                    status: WorkflowStatus {
                        workflow_id: workflow_id.clone(),
                        title: title.clone(),
                        phase: 0,
                        mode: 0,
                        overall_progress: 0.0,
                        current_agent: String::new(),
                        status_message: "Initializing".into(),
                        recent_events: vec![],
                        requires_approval: false,
                        pending_approval: None,
                    },
                    events: vec![],
                    cancelled: false,
                    approved: false,
                    mode: 0,
                },
            );
        }

        tokio::spawn(async move {
            let phases: Vec<(i32, &str, &str)> = vec![
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

            let total = phases.len() as f32;

            for (i, (phase, phase_name, agent_id)) in phases.iter().enumerate() {
                // Check cancelled
                {
                    let wf_lock = workflows.lock().await;
                    if let Some(state) = wf_lock.get(&workflow_id) {
                        if state.cancelled {
                            let event = WorkflowEvent {
                                workflow_id: workflow_id.clone(),
                                phase: 12, // CANCELLED
                                agent_id: String::new(),
                                message: "Workflow Cancelled".into(),
                                detail: "Workflow was cancelled by user.".into(),
                                progress: 0.0,
                                requires_approval: false,
                                approval: None,
                            };
                            let _ = tx.send(Ok(event)).await;
                            break;
                        }
                    }
                }

                let event = WorkflowEvent {
                    workflow_id: workflow_id.clone(),
                    phase: *phase,
                    agent_id: agent_id.to_string(),
                    message: format!("Phase: {}", phase_name),
                    detail: format!(
                        "Entering {} phase for workflow '{}'",
                        phase_name, title
                    ),
                    progress: ((i + 1) as f32 / total) * 100.0,
                    requires_approval: *phase == 7,
                    approval: if *phase == 7 {
                        Some(ApprovalRequest {
                            title: "Plan Approval Required".into(),
                            description: format!("Review the execution plan for: {}", title),
                            options: vec![
                                "Parallel".into(),
                                "Sequential".into(),
                                "Hybrid".into(),
                            ],
                            plan_summary: format!("Execution plan for: {}", description),
                            recommended_mode: 0,
                        })
                    } else {
                        None
                    },
                };

                // Send event to stream
                if tx.send(Ok(event.clone())).await.is_err() {
                    break;
                }

                // Update workflow state
                {
                    let mut wf_lock = workflows.lock().await;
                    if let Some(state) = wf_lock.get_mut(&workflow_id) {
                        state.status.phase = *phase;
                        state.status.overall_progress = event.progress;
                        state.status.current_agent = agent_id.to_string();
                        state.status.status_message = event.message.clone();
                        state.events.push(event.clone());
                        state.status.recent_events.push(event.clone());
                        if state.status.recent_events.len() > 20 {
                            state.status.recent_events.remove(0);
                        }
                        if *phase == 7 {
                            state.status.requires_approval = true;
                            state.status.pending_approval = event.approval.clone();
                        }
                    }
                }

                // Add agent log
                if !agent_id.is_empty() {
                    let mut logs = agent_logs.lock().await;
                    let key = format!("{}/{}", workflow_id, agent_id);
                    logs.entry(key).or_insert_with(Vec::new).push(AgentLogEntry {
                        timestamp: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs_f64(),
                        level: "info".into(),
                        message: format!("Starting phase: {}", phase_name),
                        detail: format!("Workflow: {} — Phase: {}", title, phase_name),
                    });
                }

                // For Awaiting Approval, wait until approved or cancelled
                if *phase == 7 {
                    for _ in 0..600 {
                        // ~5 min timeout
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        let decision = {
                            let wf_lock = workflows.lock().await;
                            wf_lock
                                .get(&workflow_id)
                                .map(|s| (s.approved, s.cancelled))
                                .unwrap_or((false, true))
                        };
                        if decision.1 {
                            // cancelled
                            break;
                        }
                        if decision.0 {
                            // approved
                            // Update mode from approval
                            if let Some(state) = workflows.lock().await.get_mut(&workflow_id) {
                                state.status.mode = state.mode;
                            }
                            break;
                        }
                    }
                } else {
                    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                }
            }
        });

        Ok(Response::new(
            Box::pin(ReceiverStream::new(rx)) as Self::StartWorkflowStream
        ))
    }

    // ─── GetWorkflowStatus ───
    async fn get_workflow_status(
        &self,
        request: Request<GetWorkflowStatusRequest>,
    ) -> Result<Response<WorkflowStatus>, Status> {
        let req = request.into_inner();
        let workflows = self.workflows.lock().await;
        let state = workflows
            .get(&req.workflow_id)
            .ok_or_else(|| Status::not_found(format!("Workflow '{}' not found", req.workflow_id)))?;
        Ok(Response::new(state.status.clone()))
    }

    // ─── ListWorkflows ───
    async fn list_workflows(
        &self,
        _request: Request<ListWorkflowsRequest>,
    ) -> Result<Response<ListWorkflowsResponse>, Status> {
        let workflows = self.workflows.lock().await;
        let statuses: Vec<WorkflowStatus> = workflows
            .values()
            .map(|s| s.status.clone())
            .collect();
        Ok(Response::new(ListWorkflowsResponse {
            workflows: statuses,
        }))
    }

    // ─── CancelWorkflow ───
    async fn cancel_workflow(
        &self,
        request: Request<CancelWorkflowRequest>,
    ) -> Result<Response<CancelWorkflowResponse>, Status> {
        let req = request.into_inner();
        let mut workflows = self.workflows.lock().await;
        if let Some(state) = workflows.get_mut(&req.workflow_id) {
            state.cancelled = true;
            state.status.phase = 12; // CANCELLED
            state.status.status_message = format!("Cancelled: {}", req.reason);
            Ok(Response::new(CancelWorkflowResponse { cancelled: true }))
        } else {
            Ok(Response::new(CancelWorkflowResponse { cancelled: false }))
        }
    }

    // ─── ApprovePlan ───
    async fn approve_plan(
        &self,
        request: Request<ApprovePlanRequest>,
    ) -> Result<Response<ApprovePlanResponse>, Status> {
        let req = request.into_inner();
        let mut workflows = self.workflows.lock().await;
        if let Some(state) = workflows.get_mut(&req.workflow_id) {
            state.approved = req.approved;
            state.mode = req.mode;
            state.status.requires_approval = false;
            state.status.pending_approval = None;

            if req.approved {
                state.status.status_message =
                    format!("Plan approved — executing in mode {}", req.mode);
            } else {
                state.status.status_message =
                    format!("Plan rejected: {}", req.feedback);
            }

            Ok(Response::new(ApprovePlanResponse {
                accepted: req.approved,
                message: state.status.status_message.clone(),
            }))
        } else {
            Err(Status::not_found(format!(
                "Workflow '{}' not found",
                req.workflow_id
            )))
        }
    }

    // ─── ListAgents ───
    async fn list_agents(
        &self,
        _request: Request<ListAgentsRequest>,
    ) -> Result<Response<ListAgentsResponse>, Status> {
        let agents = self.agents.lock().await;
        let proto_agents: Vec<AgentInfo> = agents
            .iter()
            .map(|a| AgentInfo {
                agent_id: a.agent_id.clone(),
                name: a.name.clone(),
                role: a.role.clone(),
                status: a.status.clone(),
                current_task: a.current_task.clone(),
                progress: a.progress,
            })
            .collect();
        Ok(Response::new(ListAgentsResponse {
            agents: proto_agents,
        }))
    }

    // ─── GetAgentLog (server-streaming) ───
    type GetAgentLogStream = LogStream;

    async fn get_agent_log(
        &self,
        request: Request<GetAgentLogRequest>,
    ) -> Result<Response<Self::GetAgentLogStream>, Status> {
        let req = request.into_inner();
        let key = format!("{}/{}", req.workflow_id, req.agent_id);
        let (tx, rx) = mpsc::channel(128);

        let logs = self.agent_logs.lock().await;
        let entries = logs.get(&key).cloned().unwrap_or_default();
        drop(logs);

        tokio::spawn(async move {
            for entry in entries {
                if tx.send(Ok(entry)).await.is_err() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        });

        Ok(Response::new(
            Box::pin(ReceiverStream::new(rx)) as Self::GetAgentLogStream
        ))
    }
}

// ─── Main ───
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let _ = env_logger::try_init();
    info!("Starting Steward Daemon...");

    let addr = "127.0.0.1:50051".parse()?;
    let steward = MySteward::new("steward.db")?;

    info!("Steward Daemon listening on {}", addr);

    Server::builder()
        .accept_http1(true)
        .layer(CorsLayer::permissive())
        .layer(GrpcWebLayer::new())
        .add_service(StewardServiceServer::new(steward))
        .serve(addr)
        .await?;

    Ok(())
}
