//! `steward-coding`: repository intelligence (Omega §25, Phase 9-10).
//!
//! Repo index with hash-cache incremental reindex, unified resource reader,
//! hash-anchored edit engine with stale detection, heuristic symbol graph
//! with ranked repo map, and LSP manager over pluggable transports.

pub mod dap;
pub mod edit;
pub mod index;
pub mod lsp;
pub mod reader;
pub mod repomap;

#[cfg(test)]
#[path = "dap_tests.rs"]
mod dap_tests;

#[cfg(test)]
#[path = "lsp_tests.rs"]
mod lsp_tests;

pub use dap::{DapManager, DebugSession, StackFrame, Variable};
pub use edit::{AnchoredEdit, EditAnchor, EditEngine, EditError};
pub use index::{FileEntry, RepoIndex};
pub use lsp::{Diagnostic, DiagnosticSeverity, Location, LspManager, LspSession};
pub use reader::ResourceReader;
pub use repomap::{RepoMap, SymbolEntry};
