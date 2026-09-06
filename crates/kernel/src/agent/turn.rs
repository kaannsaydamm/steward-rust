//! The TurnEngine state machine (Task 4.2 / §15).
//!
//! Build → Model → Normalize actions → (policy/budget) → Execute → Observe →
//! Success check. No fixed round count: the loop ends on Final action,
//! success policy, budget exhaustion, or error — plus a configurable
//! emergency ceiling.

use crate::action::AgentAction;
use crate::budget::{Budget, BudgetCheck};
use crate::event::KernelEvent;
use crate::services::{
    KernelMessage, KernelServices, ModelRequest, ModelResponse, Observation, ToolSchema,
};
use crate::success::{SuccessEvaluator, SuccessPolicy, SuccessVerdict};
use anyhow::{bail, Result};

/// Observable outcome of a completed run.
#[derive(Clone, Debug, PartialEq)]
pub enum TurnOutcome {
    Completed { final_text: String },
    Failed { reason: String },
    /// Emergency ceiling hit; distinguishable from budgeted termination.
    CeilingExceeded,
}

/// Result of executing one action.
#[derive(Clone, Debug)]
pub struct ActionResult {
    pub call_id: String,
    pub ok: bool,
    pub observation: String,
}

pub struct TurnEngine {
    /// Configurable emergency hard ceiling (never the primary mechanism).
    pub emergency_ceiling: u32,
}

impl Default for TurnEngine {
    fn default() -> Self {
        Self { emergency_ceiling: 64 }
    }
}

impl TurnEngine {
    /// Drives the loop to termination. `history` is the current conversation;
    /// executed tool observations are appended as canonical tool messages.
    pub async fn run(
        &self,
        services: &KernelServices,
        history: Vec<KernelMessage>,
        tools: Vec<ToolSchema>,
        budget: &Budget,
        policy: &SuccessPolicy,
    ) -> Result<TurnOutcome> {
        let mut history = history;
        let mut model_calls: u32 = 0;
        let mut tool_calls: u32 = 0;
        let mut failures: u32 = 0;
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;
        let mut cost: u64 = 0;
        let started = std::time::Instant::now();

        services.events.emit(KernelEvent::RunStarted).await;

        let mut turn: u32 = 0;
        loop {
            turn += 1;
            if turn > self.emergency_ceiling {
                services
                    .events
                    .emit(KernelEvent::RunFailed { reason: "emergency ceiling".into() })
                    .await;
                return Ok(TurnOutcome::CeilingExceeded);
            }

            services.events.emit(KernelEvent::TurnStarted { turn }).await;

            // Budget gate before each generation.
            match budget.check(
                model_calls,
                tool_calls,
                0,
                failures,
                input_tokens,
                output_tokens,
                cost,
                started.elapsed().as_millis() as u64,
            ) {
                BudgetCheck::Ok => {}
                BudgetCheck::Exhausted(dimension) => {
                    services
                        .events
                        .emit(KernelEvent::BudgetExhausted { dimension: dimension.to_owned() })
                        .await;
                    return Ok(TurnOutcome::Failed { reason: format!("budget exhausted: {dimension}") });
                }
            }

            // Generate.
            let request = ModelRequest { messages: history.clone(), tools: tools.clone() };
            let response: ModelResponse = match services.models.complete(request).await {
                Ok(response) => response,
                Err(error) => {
                    failures += 1;
                    services
                        .events
                        .emit(KernelEvent::RunFailed { reason: error.to_string() })
                        .await;
                    return Ok(TurnOutcome::Failed { reason: error.to_string() });
                }
            };
            model_calls += 1;
            input_tokens += response.prompt_tokens;
            output_tokens += response.completion_tokens;
            cost += response.cost_microusd;
            services
                .events
                .emit(KernelEvent::ModelCallCompleted {
                    prompt_tokens: response.prompt_tokens,
                    completion_tokens: response.completion_tokens,
                })
                .await;

            if !response.text.is_empty() {
                services
                    .events
                    .emit(KernelEvent::AssistantText { text: response.text.clone() })
                    .await;
                history.push(KernelMessage {
                    role: "assistant".into(),
                    content: response.text.clone(),
                    tool_call_id: None,
                });
            }

            // Normalize actions; a Final action ends the run.
            let mut executed_tool_observations: Vec<crate::services::ToolOutcome> = Vec::new();
            let mut final_text: Option<String> = None;
            for action in &response.actions {
                match action {
                    AgentAction::Final { text } => {
                        final_text = Some(text.clone());
                    }
                    AgentAction::Tool { call_id, name, .. } => {
                        services
                            .events
                            .emit(KernelEvent::ToolStarted { call_id: call_id.clone(), name: name.clone() })
                            .await;
                        let outcome = match services.tools.execute(action).await {
                            Ok(outcome) => outcome,
                            Err(error) => {
                                failures += 1;
                                crate::services::ToolOutcome {
                                    call_id: call_id.clone(),
                                    ok: false,
                                    observation: format!("tool error: {error}"),
                                }
                            }
                        };
                        tool_calls += 1;
                        services
                            .events
                            .emit(KernelEvent::ToolCompleted { call_id: call_id.clone(), ok: outcome.ok })
                            .await;
                        executed_tool_observations.push(outcome);
                    }
                    AgentAction::Code { .. } | AgentAction::Spawn { .. } => {
                        // Phases 11/14 wire these executors; treat as observed
                        // no-ops until then so the loop stays small.
                    }
                }
            }

            // Canonical observations: tool results enter history exactly once.
            for outcome in executed_tool_observations {
                history.push(KernelMessage {
                    role: "tool".into(),
                    content: outcome.observation,
                    tool_call_id: Some(outcome.call_id),
                });
            }

            // Termination: Final action present.
            if let Some(text) = final_text {
                services
                    .events
                    .emit(KernelEvent::RunCompleted { final_text: text.clone() })
                    .await;
                return Ok(TurnOutcome::Completed { final_text: text });
            }

            // A text-only reply with no actions is a natural completion
            // (AgentDeclared policy) — same as v1 behavior, minus the round cap.
            if response.actions.is_empty() {
                if response.text.is_empty() {
                    failures += 1;
                    if failures > 1 {
                        bail!("agent produced empty response twice");
                    }
                    continue;
                }
                let final_text = response.text.clone();
                services
                    .events
                    .emit(KernelEvent::RunCompleted { final_text: final_text.clone() })
                    .await;
                return Ok(TurnOutcome::Completed { final_text });
            }

            // Success policy gate (only meaningful for non-AgentDeclared).
            if !matches!(policy, SuccessPolicy::AgentDeclared) {
                let last_text = history
                    .iter()
                    .rev()
                    .find(|m| m.role == "assistant")
                    .map(|m| m.content.clone())
                    .unwrap_or_default();
                match services.success.evaluate(policy, &last_text).await? {
                    SuccessVerdict::Satisfied => {
                        let final_text = last_text;
                        services
                            .events
                            .emit(KernelEvent::RunCompleted { final_text: final_text.clone() })
                            .await;
                        return Ok(TurnOutcome::Completed { final_text });
                    }
                    _ => {}
                }
            }
        }
    }

    /// Observation projection used by Context Engine integration later; the
    /// canonical history already contains each tool result exactly once (K-002).
    pub fn observations(history: &[KernelMessage]) -> Observation {
        Observation { messages: history.to_vec() }
    }
}
