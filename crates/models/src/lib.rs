//! `steward-models`: normalized model layer v2 (Omega §21, M-006..M-008).
//!
//! Capability metadata, task-aware routing where hard capability constraints
//! always beat soft scores, and policy-driven fallback ladders. Provider
//! HTTP stays in the daemon; this crate owns routing semantics only.

pub mod catalog;
pub mod fallback;
pub mod health;
pub mod router;

pub use catalog::{ModelCapabilities, ModelEntry, RouteId};
pub use fallback::{FallbackPolicy, FallbackRule};
pub use health::{HealthState, ProviderHealth};
pub use router::{RouteRequest, Router, SelectedRoute, TaskKind};
