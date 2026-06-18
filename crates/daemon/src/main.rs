use anyhow::{Context as _, Result};
use log::info;
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use steward_core::pb::steward_service_server::StewardServiceServer;
use steward_core::pb::AgentLogEntry;
use steward_knowledge::KnowledgeEngine;
use tonic::transport::Server;
use tonic_web::GrpcWebLayer;
use tower_http::cors::CorsLayer;
use wasmtime::Engine;

type SqliteExtensionEntry = unsafe extern "C" fn(
    *mut rusqlite::ffi::sqlite3,
    *mut *mut std::ffi::c_char,
    *const rusqlite::ffi::sqlite3_api_routines,
) -> std::ffi::c_int;

mod mcp_lifecycle;
mod mcp_protocol;
mod mcp_registry;
mod mcp_runtime;
mod mcp_session;
mod nightly;
mod rpc;
mod service;
mod skill_installation;
mod state;
mod tool_audit;
mod tool_executors;
mod tool_invocation;
mod tool_policy;
mod tool_registry;
mod workflow_events;
mod workflow_logs;
mod workflow_runtime;
mod workflow_store;

#[derive(Clone)]
pub struct MySteward {
    db: Arc<Mutex<Connection>>,
    wasm_engine: Engine,
    knowledge: Arc<KnowledgeEngine>,
    workflows: Arc<tokio::sync::Mutex<HashMap<String, state::WorkflowState>>>,
    agents: Arc<tokio::sync::Mutex<Vec<state::InternalAgent>>>,
    agent_logs: Arc<tokio::sync::Mutex<HashMap<String, Vec<AgentLogEntry>>>>,
    mcp_runtime: Arc<mcp_runtime::McpRuntime>,
}

impl MySteward {
    pub fn new(db_path: &str) -> Result<Self> {
        // SAFETY: Categories 8/13 (FFI and library contract). sqlite-vec exports
        // this symbol as SQLite's extension entrypoint and its crate documents
        // this exact registration cast. Registration occurs before opening the
        // connections that use the process-global auto-extension callback.
        unsafe {
            let entry = std::mem::transmute::<*const (), SqliteExtensionEntry>(
                sqlite_vec::sqlite3_vec_init as *const (),
            );
            rusqlite::ffi::sqlite3_auto_extension(Some(entry));
        }

        let db = Connection::open(db_path)?;
        db.execute(
            "CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY,
                task TEXT NOT NULL
            )",
            [],
        )?;
        let knowledge = KnowledgeEngine::new(db)?;

        let direct_db = Connection::open(db_path)?;
        workflow_store::create_schema(&direct_db)?;
        tool_registry::initialize(&direct_db)?;
        mcp_registry::disable_all_tools(&direct_db)?;
        let persisted_workflows = workflow_store::load_workflows(&direct_db)?;

        Ok(Self {
            db: Arc::new(Mutex::new(direct_db)),
            wasm_engine: Engine::default(),
            knowledge: Arc::new(knowledge),
            workflows: Arc::new(tokio::sync::Mutex::new(persisted_workflows)),
            agents: Arc::new(tokio::sync::Mutex::new(state::default_agents())),
            agent_logs: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            mcp_runtime: Arc::new(mcp_runtime::McpRuntime::new()),
        })
    }

    pub(crate) fn workflow_runtime(&self) -> workflow_runtime::WorkflowRuntime {
        workflow_runtime::WorkflowRuntime::new(
            self.db.clone(),
            self.workflows.clone(),
            self.agent_logs.clone(),
        )
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let _ = env_logger::try_init();
    info!("Starting Steward Daemon...");

    let config = nightly::daemon_config()?;
    let storage_root = steward_core::storage::root().context("resolving ~/.steward data root")?;
    std::fs::create_dir_all(&storage_root)?;
    let database_path = storage_root.join("steward.db");
    let steward = MySteward::new(
        database_path
            .to_str()
            .context("Steward database path is not valid UTF-8")?,
    )?;
    steward.workflow_runtime().resume_persisted();
    if config.dream_now {
        let report = nightly::create_nightly_dream_file(&steward.knowledge, &config.dream_dir)?;
        info!("Nightly dream written: {}", report.display());
    }
    nightly::spawn_nightly_dream_scheduler(steward.knowledge.clone(), config.dream_dir.clone());

    info!("Steward Daemon listening on {}", config.addr);
    Server::builder()
        .accept_http1(true)
        .layer(CorsLayer::permissive())
        .layer(GrpcWebLayer::new())
        .add_service(StewardServiceServer::new(steward))
        .serve(config.addr)
        .await?;
    Ok(())
}
