use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use wasmtime::{Config, Engine, Instance, Module, Store, StoreLimits, StoreLimitsBuilder};

/// Hard ceiling for a module read from the workspace. The file executor checks metadata before
/// reading it and this module checks the final byte buffer again to close the race between metadata
/// and read.
pub const MAX_WASM_MODULE_BYTES: usize = 16 * 1024 * 1024;

/// Per-store limits. Steward exposes no host imports, but an untrusted pure-compute module can still
/// consume unbounded CPU or reserve large memories/tables without these limits.
const MAX_WASM_MEMORY_BYTES: usize = 64 * 1024 * 1024;
const MAX_WASM_TABLE_ELEMENTS: u32 = 10_000;
const MAX_WASM_INSTANCES: usize = 1;
const MAX_WASM_MEMORIES: usize = 1;
const MAX_WASM_TABLES: usize = 2;
const MAX_WASM_FUEL: u64 = 10_000_000;

struct SandboxState {
    limits: StoreLimits,
}

/// Builds the shared engine used by every Steward WASM entry point. Fuel consumption must be
pub fn build_engine() -> Result<Engine> {
    let mut config = Config::new();
    config.consume_fuel(true);
    Engine::new(&config).context("creating bounded WASM engine")
}

/// Executes the deliberately small Steward plugin ABI: a module with no imports and one exported
/// `run() -> i32` function.
pub fn execute(engine: &Engine, bytes: &[u8]) -> Result<i32> {
    if bytes.len() > MAX_WASM_MODULE_BYTES {
        bail!(
            "WASM module is {} bytes and exceeds the 16 MiB limit",
            bytes.len()
        );
    }

    let module = Module::from_binary(engine, bytes).context("compiling WASM")?;
    let limits = StoreLimitsBuilder::new()
        .memory_size(MAX_WASM_MEMORY_BYTES)
        .table_elements(MAX_WASM_TABLE_ELEMENTS as usize)
        .instances(MAX_WASM_INSTANCES)
        .memories(MAX_WASM_MEMORIES)
        .tables(MAX_WASM_TABLES)
        .trap_on_grow_failure(true)
        .build();
    let mut store = Store::new(engine, SandboxState { limits });
    store.limiter(|state| &mut state.limits);
    store
        .set_fuel(MAX_WASM_FUEL)
        .context("configuring WASM execution budget")?;

    let instance = Instance::new(&mut store, &module, &[])
        .context("instantiating WASM (modules with imports are not supported)")?;
    let run = instance
        .get_typed_func::<(), i32>(&mut store, "run")
        .context("module does not export run() -> i32")?;

    match run.call(&mut store, ()) {
        Ok(result) => Ok(result),
        Err(error) => {
            if matches!(store.get_fuel(), Ok(0)) {
                bail!("WASM execution budget exhausted");
            }
            Err(error).context("executing run()")
        }
    }
}

/// Resolves and runs a workspace module without blocking the async runtime. This is the only path
/// used by the governed `wasm.run` tool.
pub async fn execute_workspace_file(
    engine: &Engine,
    arguments: &BTreeMap<String, String>,
    working_directory: &str,
) -> Result<String> {
    let path_argument = arguments
        .get("path")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("path is required")?;
    let root = workspace_root(working_directory)?;
    let resolved = resolve_workspace_file(&root, path_argument)?;
    if !resolved.is_file() {
        bail!("'{path_argument}' is not a file");
    }
    let metadata = tokio::fs::metadata(&resolved)
        .await
        .with_context(|| format!("reading metadata for {path_argument}"))?;
    if metadata.len() > MAX_WASM_MODULE_BYTES as u64 {
        bail!("WASM module exceeds the 16 MiB limit");
    }
    let bytes = tokio::fs::read(&resolved)
        .await
        .with_context(|| format!("reading {path_argument}"))?;
    let engine = engine.clone();
    let result = tokio::task::spawn_blocking(move || execute(&engine, &bytes))
        .await
        .context("WASM task panicked")??;
    Ok(format!("run() returned {result}"))
}

fn workspace_root(working_directory: &str) -> Result<PathBuf> {
    let root = if working_directory.trim().is_empty() {
        std::env::current_dir().context("resolving current directory")?
    } else {
        PathBuf::from(working_directory)
    };
    root.canonicalize()
        .with_context(|| format!("resolving working directory {}", root.display()))
}

fn resolve_workspace_file(root: &Path, relative: &str) -> Result<PathBuf> {
    let candidate = Path::new(relative);
    if candidate.is_absolute() {
        bail!("path '{relative}' must be relative to the working directory");
    }
    let resolved = root
        .join(candidate)
        .canonicalize()
        .with_context(|| format!("resolving path {relative}"))?;
    if !resolved.starts_with(root) {
        bail!("path '{relative}' escapes the working directory");
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::{
        build_engine, execute, execute_workspace_file, resolve_workspace_file,
        MAX_WASM_MODULE_BYTES,
    };
    use std::collections::BTreeMap;

    #[test]
    fn bounded_module_returns_its_result() {
        let engine = build_engine().expect("configured WASM engine");
        let module = wat::parse_str(
            r#"(module
                (func (export "run") (result i32)
                    i32.const 7))"#,
        )
        .expect("valid test module");

        assert_eq!(execute(&engine, &module).expect("execute module"), 7);
    }

    #[test]
    fn infinite_module_exhausts_its_execution_budget() {
        let engine = build_engine().expect("configured WASM engine");
        let module = wat::parse_str(
            r#"(module
                (func (export "run") (result i32)
                    (loop $forever
                        br $forever)
                    i32.const 0))"#,
        )
        .expect("valid test module");

        let error = execute(&engine, &module).expect_err("infinite module must be stopped");

        assert!(
            error.to_string().contains("execution budget exhausted"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn oversized_module_is_rejected_before_compilation() {
        let engine = build_engine().expect("configured WASM engine");
        let module = vec![0_u8; MAX_WASM_MODULE_BYTES + 1];

        let error = execute(&engine, &module).expect_err("oversized module must be rejected");

        assert!(error.to_string().contains("exceeds the 16 MiB limit"));
    }

    #[test]
    fn module_cannot_reserve_more_than_the_memory_budget() {
        let engine = build_engine().expect("configured WASM engine");
        // WebAssembly pages are 64 KiB; 1025 pages exceeds the 64 MiB store limit.
        let module = wat::parse_str(
            r#"(module
                (memory 1025)
                (func (export "run") (result i32)
                    i32.const 0))"#,
        )
        .expect("valid test module");

        execute(&engine, &module).expect_err("oversized memory must be rejected");
    }

    #[test]
    fn workspace_resolution_rejects_escape_and_absolute_paths() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("create workspace");
        let outside = temp.path().join("outside.wasm");
        std::fs::write(&outside, b"outside").expect("write outside");

        assert!(resolve_workspace_file(&workspace, "../outside.wasm").is_err());
        assert!(resolve_workspace_file(&workspace, outside.to_str().unwrap()).is_err());
    }

    #[tokio::test]
    async fn workspace_executor_runs_a_confined_module() {
        let temp = tempfile::tempdir().expect("tempdir");
        let module = wat::parse_str(
            r#"(module
                (func (export "run") (result i32)
                    i32.const 9))"#,
        )
        .expect("valid test module");
        std::fs::write(temp.path().join("plugin.wasm"), module).expect("write module");
        let mut arguments = BTreeMap::new();
        arguments.insert("path".to_owned(), "plugin.wasm".to_owned());
        let engine = build_engine().expect("configured WASM engine");

        let output = execute_workspace_file(&engine, &arguments, temp.path().to_str().unwrap())
            .await
            .expect("execute workspace module");

        assert_eq!(output, "run() returned 9");
    }
}
