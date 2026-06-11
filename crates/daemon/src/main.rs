use anyhow::Result;
use log::{error, info};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use steward_core::pb::steward_service_server::{StewardService, StewardServiceServer};
use steward_core::pb::{ExecuteTaskRequest, ExecuteTaskResponse, RunPluginRequest, RunPluginResponse};
use tonic::{transport::Server, Request, Response, Status};
use wasmtime::{Engine, Instance, Module, Store};

#[derive(Clone)]
pub struct MySteward {
    db: Arc<Mutex<Connection>>,
    wasm_engine: Engine,
}

impl MySteward {
    pub fn new(db_path: &str) -> Result<Self> {
        // Init sqlite-vec auto extension
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const ()
            )));
        }

        let db = Connection::open(db_path)?;
        
        // Initialize tasks table
        db.execute(
            "CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY,
                task TEXT NOT NULL
            )",
            [],
        )?;

        let wasm_engine = Engine::default();

        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            wasm_engine,
        })
    }
}

#[tonic::async_trait]
impl StewardService for MySteward {
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

        // Assuming the module has a function named `run` that returns an i32
        let run_func = instance.get_typed_func::<(), i32>(&mut store, "run")
            .map_err(|e| Status::internal(format!("WASM missing 'run' function: {}", e)))?;

        let result = run_func.call(&mut store, ())
            .map_err(|e| Status::internal(format!("Failed to execute 'run': {}", e)))?;

        Ok(Response::new(RunPluginResponse {
            output: format!("Plugin executed successfully with result: {}", result),
        }))
    }

    async fn execute_task(
        &self,
        request: Request<ExecuteTaskRequest>,
    ) -> Result<Response<ExecuteTaskResponse>, Status> {
        let req = request.into_inner();
        
        let db = self.db.lock().map_err(|_| Status::internal("Database lock failed"))?;
        db.execute(
            "INSERT INTO tasks (task) VALUES (?1)",
            [&req.task],
        ).map_err(|e| Status::internal(format!("Failed to save task: {}", e)))?;

        info!("Task executed and saved: {}", req.task);

        Ok(Response::new(ExecuteTaskResponse {
            status: "Task stored in SQLite successfully".into(),
        }))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    info!("Starting Steward Daemon...");

    let addr = "127.0.0.1:50051".parse()?;
    let steward = MySteward::new("steward.db")?;

    info!("Steward Daemon listening on {}", addr);

    Server::builder()
        .add_service(StewardServiceServer::new(steward))
        .serve(addr)
        .await?;

    Ok(())
}
