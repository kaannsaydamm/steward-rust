//! Deterministic evaluation harness for Steward.
//!
//! Owns the scripted fake model used by kernel/runtime tests, task fixtures,
//! graders, and the parity registry (Phase 0 of the Omega plan).

pub mod fake_model;
pub mod parity;
#[cfg(test)]
mod tests;

pub use fake_model::{ScriptedModel, ScriptedReply};
pub use parity::{ParityCapability, PARITY_REGISTRY};
