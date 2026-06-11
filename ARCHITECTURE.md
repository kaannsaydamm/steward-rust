# Architecture
Steward is a multi-channel Agent OS featuring a pure Rust core daemon, an embedded Wasmtime/Extism plugin sandbox, an SQLite/sqlite-vec RAG memory, and a LangGraph-style stateful DAG engine.
- `steward-daemon`: Pure Rust background service exposing a local gRPC/protobuf API.
- `steward-cli`: Lightweight command-line tool implemented in Rust connecting to the daemon.
- `steward-core`: Shared library crate.
- `steward.db`: SQLite database storing vector memory using `sqlite-vec`.
