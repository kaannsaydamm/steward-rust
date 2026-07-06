use crate::MySteward;
use log::info;
use steward_core::pb::{
    ChatRequest, ExecuteTaskRequest, ExecuteTaskResponse, PingRequest, PingResponse,
    RunPluginRequest, RunPluginResponse,
};
use tonic::{Request, Response, Status};
use wasmtime::{Instance, Module, Store};

pub async fn ping(_request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
    Ok(Response::new(PingResponse {
        status: "OK".into(),
    }))
}

pub async fn run_plugin(
    steward: &MySteward,
    request: Request<RunPluginRequest>,
) -> Result<Response<RunPluginResponse>, Status> {
    let wasm_bytes = request.into_inner().wasm_binary;
    if wasm_bytes.is_empty() {
        return Err(Status::invalid_argument("WASM binary is empty"));
    }

    let module = Module::from_binary(&steward.wasm_engine, &wasm_bytes)
        .map_err(|error| Status::internal(format!("Failed to compile WASM: {error}")))?;
    let mut store = Store::new(&steward.wasm_engine, ());
    let instance = Instance::new(&mut store, &module, &[])
        .map_err(|error| Status::internal(format!("Failed to instantiate WASM: {error}")))?;
    let run_func = instance
        .get_typed_func::<(), i32>(&mut store, "run")
        .map_err(|error| Status::internal(format!("WASM missing 'run' function: {error}")))?;
    let result = run_func
        .call(&mut store, ())
        .map_err(|error| Status::internal(format!("Failed to execute 'run': {error}")))?;

    Ok(Response::new(RunPluginResponse {
        output: format!("Plugin executed successfully with result: {result}"),
    }))
}

pub async fn execute_task(
    steward: &MySteward,
    request: Request<ExecuteTaskRequest>,
) -> Result<Response<ExecuteTaskResponse>, Status> {
    let task = request.into_inner().task;
    let working_directory = std::env::current_dir()
        .map_err(|error| Status::internal(format!("Failed to resolve working directory: {error}")))?
        .display()
        .to_string();
    let (sender, _receiver) = tokio::sync::mpsc::channel(64);
    let result = crate::agent_runtime::run(
        steward,
        ChatRequest {
            session_id: String::new(),
            message: task.clone(),
            working_directory,
            allow_tools: true,
        },
        sender,
    )
    .await
    .map_err(|error| Status::failed_precondition(format!("{error:#}")))?;
    let db = steward
        .db
        .lock()
        .map_err(|_| Status::internal("Database lock failed"))?;
    db.execute("INSERT INTO tasks (task) VALUES (?1)", [&task])
        .map_err(|error| Status::internal(format!("Failed to save task: {error}")))?;
    info!("Task completed in session {}", result.session_id);
    Ok(Response::new(ExecuteTaskResponse {
        status: result.final_text,
    }))
}
