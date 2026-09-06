//! Kernel-emitted events; the daemon persists them into `run_events`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum KernelEvent {
    RunStarted,
    TurnStarted {
        turn: u32,
    },
    AssistantText {
        text: String,
    },
    ModelCallCompleted {
        prompt_tokens: u64,
        completion_tokens: u64,
    },
    ActionProposed {
        action: String,
    },
    ToolStarted {
        call_id: String,
        name: String,
    },
    ToolCompleted {
        call_id: String,
        ok: bool,
    },
    Checkpointed {
        checkpoint_id: String,
    },
    AwaitingApproval {
        approval_id: String,
    },
    RunCompleted {
        final_text: String,
    },
    RunFailed {
        reason: String,
    },
    BudgetExhausted {
        dimension: String,
    },
}
