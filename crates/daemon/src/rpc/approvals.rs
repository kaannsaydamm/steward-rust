//! Chat-visible approval surface over the kernel HITL registry.
//!
//! Ported from NousResearch/hermes-agent@693641aa8b4359c602283bdbbc14041e03bc47bc
//! `approval.respond` gateway semantics (MIT). Modified for Steward: pending
//! approvals live in `kernel::hitl::InterruptRegistry`; daemon RPCs enumerate
//! and resolve them.

use crate::MySteward;
use steward_core::pb::{
    ApprovalDecision, ListPendingApprovalsRequest, ListPendingApprovalsResponse, PendingApproval,
    RespondApprovalRequest, RespondApprovalResponse,
};
use steward_kernel::hitl::Interrupt;
use tonic::{Request, Response, Status};

fn pending_to_proto(interrupt_id: &str, interrupt: &Interrupt) -> Option<PendingApproval> {
    match interrupt {
        Interrupt::Approval { node, effect } => Some(PendingApproval {
            approval_id: interrupt_id.to_owned(),
            session_id: String::new(),
            tool_name: node.clone(),
            summary: format!("approval required for effect '{effect}' on node '{node}'"),
            created_at: 0.0,
        }),
        Interrupt::HumanInput { node, prompt, .. } => Some(PendingApproval {
            approval_id: interrupt_id.to_owned(),
            session_id: String::new(),
            tool_name: node.clone(),
            summary: prompt.clone(),
            created_at: 0.0,
        }),
        Interrupt::BreakpointBefore { node } | Interrupt::BreakpointAfter { node } => {
            Some(PendingApproval {
                approval_id: interrupt_id.to_owned(),
                session_id: String::new(),
                tool_name: node.clone(),
                summary: "user breakpoint".to_owned(),
                created_at: 0.0,
            })
        }
    }
}

/// Lists every interrupt currently paused in the HITL registry, enumerating
/// pending interrupts via the registry snapshot accessor.
pub async fn list_pending(
    steward: &MySteward,
    _request: Request<ListPendingApprovalsRequest>,
) -> Result<Response<ListPendingApprovalsResponse>, Status> {
    let approvals = steward
        .interrupts
        .pending_snapshot()
        .iter()
        .filter_map(|(id, interrupt)| pending_to_proto(id, interrupt))
        .collect();
    Ok(Response::new(ListPendingApprovalsResponse { approvals }))
}

pub async fn respond(
    steward: &MySteward,
    request: Request<RespondApprovalRequest>,
) -> Result<Response<RespondApprovalResponse>, Status> {
    let request = request.into_inner();
    let decision =
        ApprovalDecision::try_from(request.decision).unwrap_or(ApprovalDecision::Unspecified);
    let approved = !matches!(
        decision,
        ApprovalDecision::Deny | ApprovalDecision::Unspecified
    );
    // Allow-always persists the decision for the (tool, effect) pair so later
    // pauses for the same tool auto-resolve approved (scoped approval store).
    let always = decision == ApprovalDecision::AllowAlways;
    if always {
        if let Some(interrupt) = steward.interrupts.get_pending(&request.approval_id) {
            if let Interrupt::Approval { node, effect } = interrupt {
                let _ = steward.interrupts.remember_always(&node, &effect);
            }
        }
    }
    match steward.interrupts.resolve(
        &request.approval_id,
        approved,
        None,
        if always { "web:always" } else { "web" },
    ) {
        Ok(_) => Ok(Response::new(RespondApprovalResponse { accepted: true })),
        Err(_) => Ok(Response::new(RespondApprovalResponse { accepted: false })),
    }
}
