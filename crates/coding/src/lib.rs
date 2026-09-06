//! `steward-coding`: repository intelligence (Omega §25, Phase 9-10).
//!
//! Repo index with hash-cache incremental reindex, unified resource reader,
//! hash-anchored edit engine with stale detection, tree-sitter-free symbol
//! graph (heuristic scanner), and ranked repo map within a token budget.

pub mod edit;
pub mod index;
pub mod reader;
pub mod repomap;

pub use edit::{AnchoredEdit, EditAnchor, EditEngine, EditError};
pub use index::{FileEntry, RepoIndex};
pub use reader::ResourceReader;
pub use repomap::{RepoMap, SymbolEntry};
