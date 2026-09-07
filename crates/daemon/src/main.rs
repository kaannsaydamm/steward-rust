use anyhow::{Context as _, Result};
use log::info;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use steward_core::pb::steward_service_server::StewardServiceServer;
use steward_core::pb::AgentLogEntry;
use steward_knowledge::KnowledgeEngine;
use tonic::transport::Server;
use tonic_web::GrpcWebLayer;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use wasmtime::Engine;

type SqliteExtensionEntry = unsafe extern "C" fn(
    *mut rusqlite::ffi::sqlite3,
    *mut *mut std::ffi::c_char,
    *const rusqlite::ffi::sqlite3_api_routines,
) -> std::ffi::c_int;

mod agent_runtime;
mod app_server;
mod artifacts;
mod cron_jobs;
mod db_migrator;
mod file_checkpoints;
mod import_export;
mod kernel_adapter;
mod maintenance;
mod marketplace_client;
mod mcp_catalog;
mod mcp_lifecycle;
mod mcp_protocol;
mod mcp_registry;
mod mcp_runtime;
mod mcp_session;
mod nightly;
mod provider_client;
mod pty_terminal;
mod replay;
mod rpc;
mod service;
mod session_store;
mod skill_installation;
mod state;
mod stores;
mod telegram_bridge;
mod tool_audit;
mod tool_executors;
mod tool_invocation;
mod tool_policy;
mod tool_registry;
mod wasm_sandbox;
mod web_ui;
mod workflow_definition;
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
    retention: maintenance::RetentionConfig,
    provider_path: PathBuf,
    security_path: PathBuf,
    secrets: std::sync::Arc<dyn steward_core::secrets::SecretStore>,
    http: reqwest::Client,
    /// HITL pause registry backing the approvals RPC surface.
    pub(crate) interrupts: std::sync::Arc<steward_kernel::hitl::InterruptRegistry>,
}

impl MySteward {
    pub fn new(db_path: &str, retention: maintenance::RetentionConfig) -> Result<Self> {
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

        let mut direct_db = Connection::open(db_path)?;
        workflow_store::create_schema(&direct_db)?;
        workflow_definition::create_schema(&direct_db)?;
        session_store::create_schema(&direct_db)?;
        tool_registry::initialize(&direct_db)?;
        cron_jobs::create_schema(&direct_db)?;
        artifacts::create_schema(&direct_db)?;
        file_checkpoints::create_schema(&direct_db)?;
        mcp_registry::disable_all_tools(&direct_db)?;
        // Omega §48: versioned migrations with a pre-migration backup.
        let data_root_for_migrations = Path::new(db_path).parent().map(Path::to_path_buf);
        if let Some(root) = data_root_for_migrations {
            let applied = db_migrator::migrate(&mut direct_db, &root)?;
            if !applied.is_empty() {
                info!("Applied schema migrations: {:?}", applied);
            }
        }

        let persisted_workflows = workflow_store::load_workflows(&direct_db)?;

        let data_root = Path::new(db_path)
            .parent()
            .context("Steward database path has no parent directory")?;
        let provider_path = data_root.join("providers.json");
        let security_path = data_root.join("security.json");
        // Prefer the OS vault; fall back to env-only when no vault service is
        // available (headless Linux CI). `put` on the fallback fails loudly.
        let secrets: std::sync::Arc<dyn steward_core::secrets::SecretStore> =
            match steward_core::secrets::resolve(false) {
                Ok(store) => store.into(),
                Err(_) => std::sync::Arc::new(steward_core::secrets::EnvSecretStore::new()),
            };
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(120))
            .user_agent(concat!("steward/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            db: Arc::new(Mutex::new(direct_db)),
            wasm_engine: wasm_sandbox::build_engine()?,
            knowledge: Arc::new(knowledge),
            workflows: Arc::new(tokio::sync::Mutex::new(persisted_workflows)),
            agents: Arc::new(tokio::sync::Mutex::new(state::default_agents())),
            agent_logs: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            mcp_runtime: Arc::new(mcp_runtime::McpRuntime::new()),
            retention,
            provider_path,
            security_path,
            secrets,
            http,
            interrupts: Arc::new(steward_kernel::hitl::InterruptRegistry::new()),
        })
    }

    pub(crate) fn workflow_runtime(&self) -> workflow_runtime::WorkflowRuntime {
        workflow_runtime::WorkflowRuntime::new(
            self.clone(),
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
    let retention = maintenance::load_config(&storage_root)?;
    let steward = MySteward::new(
        database_path
            .to_str()
            .context("Steward database path is not valid UTF-8")?,
        retention,
    )?;
    let prune_report = {
        let connection = steward
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
        maintenance::prune(
            &connection,
            retention,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs_f64(),
        )?
    };
    info!(
        "Retention pruned {} tool invocations and {} workflows",
        prune_report.tool_invocations, prune_report.workflows
    );
    steward.workflow_runtime().resume_persisted();
    if config.dream_now {
        let report = nightly::create_nightly_dream_file(&steward.knowledge, &config.dream_dir)?;
        info!("Nightly dream written: {}", report.display());
    }
    nightly::spawn_nightly_dream_scheduler(steward.knowledge.clone(), config.dream_dir.clone());
    cron_jobs::spawn_scheduler(steward.clone());
    telegram_bridge::spawn(steward.clone(), storage_root.join("channels.json"));

    // Wire v2 app server: loopback WebSocket with an install/session token
    // persisted next to providers.json (rotated on every daemon start for
    // now; a persistent token arrives with the Desktop work, Phase 23).
    let wire_token_path = storage_root.join("wire-token");
    let wire_token = std::fs::read_to_string(&wire_token_path)
        .ok()
        .filter(|t| t.len() >= 16)
        .unwrap_or_else(|| {
            let token = uuid::Uuid::new_v4().to_string();
            let _ = std::fs::write(&wire_token_path, &token);
            token
        });
    let app_state = std::sync::Arc::new(app_server::AppState {
        auth_token: wire_token,
        ..app_server::AppState::default()
    });
    let app_router = app_server::router(app_state);
    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await;
        if let Ok(listener) = listener {
            info!(
                "Wire v2 app server on 127.0.0.1:{}",
                listener.local_addr().map(|a| a.port()).unwrap_or(0)
            );
            let _ = axum::serve(listener, app_router).await;
        }
    });

    info!("Steward Daemon listening on {}", config.addr);
    let grpc = Server::builder()
        .accept_http1(true)
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::predicate(|origin, _| {
                    origin.to_str().is_ok_and(maintenance::is_loopback_origin)
                }))
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(GrpcWebLayer::new())
        .add_service(StewardServiceServer::new(steward))
        .serve(config.addr);
    let web = web_ui::serve(config.web_addr, config.addr);
    tokio::try_join!(async { grpc.await.context("serving Steward RPC") }, web)?;
    Ok(())
}
