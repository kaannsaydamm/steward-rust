//! `steward-tools`: normalized tool runtime v2 (Omega §23, S-001..S-002).
//!
//! Tool identity is metadata; authorization is effect-based (S-001): the
//! policy evaluates declared effects + scopes, never tool names. Scoped
//! approvals carry provenance (S-002/S-003). Discovery is lazy (C-007).
//! Batch execution parallelizes only non-conflicting scopes.

pub mod approval;
pub mod batch;
pub mod discovery;
pub mod effects;
pub mod policy;
pub mod spec;

pub use approval::{ApprovalDecision, ApprovalScope, ApprovalStore, ScopedApprovalRequest};
pub use discovery::ToolIndex;
pub use effects::Effect;
pub use policy::{EffectPolicy, PolicyDecision};
pub use spec::{RiskLevel, ToolSpec};
