use anyhow::{bail, Context, Result};
use wasmtime::{Config, Engine, Instance, Module, Store, StoreLimits, StoreLimitsBuilder};

/// Hard ceiling for a module read from the workspace. The executor checks the file metadata before
/// reading it and this module checks the final byte buffer again to close the race between metadata
/// and read.
pub const MAX_WASM_MODULE_BYTES: usize = 16 * 1024 * 1024;

/// Per-store limits. `wasm.run` exposes no host imports, but an untrusted pure-compute module can
/// still consume unbounded CPU or reserve large memories/tables without these limits.
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
/// enabled on the engine before stores can receive an execution budget.
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
        .table_elements(MAX_WASM_TABLE_ELEMENTS)
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

#[cfg(test)]
mod tests {
    use super::{build_engine, execute, MAX_WASM_MODULE_BYTES};

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
}
