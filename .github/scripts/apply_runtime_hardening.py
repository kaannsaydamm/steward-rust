from pathlib import Path

path = Path("crates/daemon/src/tool_executors.rs")
source = path.read_text(encoding="utf-8")

old = '''/// Compiles and runs a workspace `.wasm` module through the daemon's shared wasmtime engine.
/// Same contract as the RunPlugin RPC: the module must export `run() -> i32`, and it runs with
/// no imports (no WASI, no host functions) — a pure sandboxed computation.
async fn run_wasm(
    steward: &MySteward,
    arguments: &BTreeMap<String, String>,
    working_directory: &str,
) -> Result<String> {
    let path_argument = arguments
        .get("path")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("path is required")?;
    let root = resolve_workspace_root(working_directory)?;
    let resolved = resolve_within_root(&root, path_argument)?;
    let bytes = tokio::fs::read(&resolved)
        .await
        .with_context(|| format!("reading {path_argument}"))?;
    let engine = steward.wasm_engine.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<i32> {
        let module = wasmtime::Module::from_binary(&engine, &bytes).context("compiling WASM")?;
        let mut store = wasmtime::Store::new(&engine, ());
        let instance = wasmtime::Instance::new(&mut store, &module, &[])
            .context("instantiating WASM (modules with imports are not supported)")?;
        let run = instance
            .get_typed_func::<(), i32>(&mut store, "run")
            .context("module does not export run() -> i32")?;
        run.call(&mut store, ()).context("executing run()")
    })
    .await
    .context("WASM task panicked")??;
    Ok(format!("run() returned {result}"))
}
'''

new = '''/// Compiles and runs a workspace `.wasm` module through the daemon's bounded sandbox.
/// Same contract as the RunPlugin RPC: the module must export `run() -> i32`, and it runs with
/// no imports (no WASI, no host functions) — a pure sandboxed computation.
async fn run_wasm(
    steward: &MySteward,
    arguments: &BTreeMap<String, String>,
    working_directory: &str,
) -> Result<String> {
    let path_argument = arguments
        .get("path")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("path is required")?;
    let root = resolve_workspace_root(working_directory)?;
    let resolved = resolve_within_root(&root, path_argument)?;
    let metadata = tokio::fs::metadata(&resolved)
        .await
        .with_context(|| format!("reading metadata for {path_argument}"))?;
    if metadata.len() > crate::wasm_sandbox::MAX_WASM_MODULE_BYTES as u64 {
        bail!("WASM module exceeds the 16 MiB limit");
    }
    let bytes = tokio::fs::read(&resolved)
        .await
        .with_context(|| format!("reading {path_argument}"))?;
    let engine = steward.wasm_engine.clone();
    let result = tokio::task::spawn_blocking(move || {
        crate::wasm_sandbox::execute(&engine, &bytes)
    })
    .await
    .context("WASM task panicked")??;
    Ok(format!("run() returned {result}"))
}
'''

if old not in source:
    raise SystemExit("expected run_wasm implementation was not found; refusing a partial patch")

path.write_text(source.replace(old, new, 1), encoding="utf-8")
