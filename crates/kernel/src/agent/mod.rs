//! Agent module root: TurnEngine + termination semantics.

pub mod turn;

#[cfg(test)]
mod tests;

pub use turn::{ActionResult, TurnEngine, TurnOutcome};
