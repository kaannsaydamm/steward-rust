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
}
