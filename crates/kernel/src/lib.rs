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
pub mod rlm;
pub mod rlm_prime;
pub mod services;
pub mod signals;
pub mod success;
pub mod trace;

#[cfg(test)]
#[path = "code_runtime_tests.rs"]
mod code_runtime_tests;

pub use action::AgentAction;
pub use budget::{Budget, BudgetCheck};
pub use event::KernelEvent;
pub use hitl::{Breakpoint, BreakpointSet, Interrupt, InterruptRegistry, InterruptResolution};
pub use services::{KernelServices, ModelService, Observation, ToolExecutor};
pub use signals::{apply_at_boundary, RunSignal, SignalBus, SignalEvent, TurnAdjustments};
pub use success::{SuccessEvaluator, SuccessPolicy, SuccessVerdict};
pub use trace::{attrs, generation_span, tool_span, Span, SpanKind, SpanStatus, TraceCollector};
