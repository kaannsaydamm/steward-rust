use anyhow::Result;
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

mod nightly;
mod rpc;
mod service;
mod state;
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
}

impl MySteward {
    pub fn new(db_path: &str) -> Result<Self> {
        // SAFETY: Categories 8/13 (FFI and library contract). sqlite-vec exports
        // this symbol as SQLite's extension entrypoint and its crate documents
        // this exact registration cast. Registration occurs before opening the
        // connections that use the process-global auto-extension callback.
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
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
        let persisted_workflows = workflow_store::load_workflows(&direct_db)?;

        Ok(Self {
            db: Arc::new(Mutex::new(direct_db)),
            wasm_engine: Engine::default(),
            knowledge: Arc::new(knowledge),
            workflows: Arc::new(tokio::sync::Mutex::new(persisted_workflows)),
            agents: Arc::new(tokio::sync::Mutex::new(state::default_agents())),
            agent_logs: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
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
    let steward = MySteward::new("steward.db")?;
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
