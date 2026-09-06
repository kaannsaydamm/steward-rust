//! `steward-context`: context engineering (Omega §19, §20, C-001..C-005).
//!
//! Owns the candidate model, the token-budgeted allocator with protected
//! lanes, per-call manifests, path-scoped rules, and observational
//! compaction. Queries knowledge/harness/coding through caller-provided
//! sources — never daemon globals.

pub mod budget;
pub mod compaction;
pub mod item;
pub mod manifest;
pub mod rules;

pub use budget::{BudgetAllocator, BudgetPolicy, PackedContext};
pub use compaction::{compact, Observation, ObservationKind, SourceMessage};
pub use item::{ContextItem, ContextScope, ContextSourceKind, Sensitivity, TrustLevel};
pub use manifest::ContextManifest;
pub use rules::{RuleBody, RuleSet};
