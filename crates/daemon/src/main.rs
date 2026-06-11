use anyhow::Result;
use rusqlite::Connection;
use steward_core::pb::steward_service_server::{StewardService, StewardServiceServer};
use steward_core::pb::{ExecuteTaskRequest, ExecuteTaskResponse, RunPluginRequest, RunPluginResponse};
use std::sync::{Arc, Mutex};
use tonic::{transport::Server, Request, Response, Status};
use wasmtime::*;

#[derive(Clone)]
pub struct DaemonService {
    db_conn: Arc<Mutex<Connection>>,
    tasks: Arc<Mutex<Vec<String>>>,
}

impl DaemonService {
    pub fn new() -> Result<Self> {
        let conn = Connection::open("steward.db")?;
        
        // Load sqlite-vec extension
        unsafe {
            conn.load_extension_enable()?;
            conn.load_extension("sqlite_vec0", None)?;
            conn.load_extension_disable()?;
        }
        
        // Create system vectors table to verify functionality
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS system_vectors USING vec0(emb float[3])",
            [],
        )?;
        
        Ok(Self {
            db_conn: Arc::new(Mutex::new(conn)),
            tasks: Arc::new(Mutex::new(Vec::new())),
        })
    }
}

#[tonic::async_trait]
impl StewardService for DaemonService {
    async fn run_plugin(
        &self,
        request: Request<RunPluginRequest>,
    ) -> Result<Response<RunPluginResponse>, Status> {
        let req = request.into_inner();
        let wasm_binary = req.wasm_binary;
        
        if wasm_binary.is_empty() {
            return Err(Status::invalid_argument("Empty WASM binary"));
        }

        let mut config = Config::new();
        config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);
        
        let engine = Engine::new(&config).map_err(|e| Status::internal(e.to_string()))?;
        let module = Module::new(&engine, &wasm_binary).map_err(|e| Status::internal(e.to_string()))?;
        
        let mut store = Store::new(&engine, ());
        let instance = Instance::new(&mut store, &module, &[]).map_err(|e| Status::internal(e.to_string()))?;
        
        let hello = instance.get_typed_func::<(), i32>(&mut store, "hello")
            .map_err(|e| Status::internal(format!("Could not find 'hello' function: {}", e)))?;
        
        let result = hello.call(&mut store, ()).map_err(|e| Status::internal(e.to_string()))?;
        
        Ok(Response::new(RunPluginResponse {
            output: format!("Plugin returned: {}", result),
        }))
    }

    async fn execute_task(
        &self,
        request: Request<ExecuteTaskRequest>,
    ) -> Result<Response<ExecuteTaskResponse>, Status> {
        let req = request.into_inner();
        
        // Actually store the task in memory
        let mut tasks = self.tasks.lock().unwrap();
        tasks.push(req.task.clone());
        let total_tasks = tasks.len();
        
        Ok(Response::new(ExecuteTaskResponse {
            status: format!("Task '{}' queued. Total tasks: {}", req.task, total_tasks),
        }))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let addr = "127.0.0.1:50051".parse()?;
    tracing::info!("Daemon starting on {}", addr);
    
    let service = DaemonService::new()?;
    
    Server::builder()
        .add_service(StewardServiceServer::new(service))
        .serve(addr)
        .await?;
        
    Ok(())
}
