//! `steward-kernel`: the small, testable agent execution core (Omega §15).
//!
//! The kernel owns only orchestration semantics: the turn state machine,
//! budgets, termination, checkpoints, events, signals. Providers, tools,
//! memory, and persistence sit behind traits — the kernel must never touch
//! HTTP, SQLite, or the filesystem directly (K-001).

pub mod action;
pub mod agent;
pub mod budget;
pub mod code_runtime;
pub mod event;
pub mod hitl;
pub mod ir;
pub mod services;
pub mod success;

pub use action::AgentAction;
pub use budget::{Budget, BudgetCheck};
pub use event::KernelEvent;
pub use hitl::{Breakpoint, BreakpointSet, Interrupt, InterruptRegistry, InterruptResolution};
pub use services::{KernelServices, ModelService, Observation, ToolExecutor};
pub use success::{SuccessEvaluator, SuccessPolicy, SuccessVerdict};
