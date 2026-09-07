//! Footer data provider: daemon computes, clients render.
//!
//! Ported from PrimeIntellect-ai/prime-agent@844e85545af6858dcb3d6cfe42bbfcf2ca0be4e5
//! `footer-data-provider.ts` split (MIT). Modified for Steward: the RPC layer
//! aggregates session/model/tool state; `—` rendering for unknown values is
//! left to the client.

use crate::MySteward;
use steward_core::pb::{FooterData, GetFooterDataRequest};
use tonic::{Request, Response, Status};

pub async fn get_footer_data(
    steward: &MySteward,
    _request: Request<GetFooterDataRequest>,
) -> Result<Response<FooterData>, Status> {
    // Active model comes from the provider settings (single source of truth
    // shared with the chat transcript header).
    let settings = steward_core::provider_config::ProviderSettings::load(&steward.provider_path)
        .map_err(|error| Status::internal(format!("{error:#}")))?;
    let (model, session_id) = match settings.active() {
        Ok(profile) => (profile.model.clone(), String::new()),
        Err(_) => (String::new(), String::new()),
    };
    // Most recently updated session id (footer shows where you are).
    let session_id = {
        let connection = steward
            .db
            .lock()
            .map_err(|_| Status::internal("Database lock failed"))?;
        crate::session_store::list_sessions(&connection, 1)
            .ok()
            .and_then(|sessions| sessions.first().map(|session| session.session_id.clone()))
            .unwrap_or_default()
    };
    // Enabled tool count from the governed tool registry.
    let tool_count = {
        let connection = steward
            .db
            .lock()
            .map_err(|_| Status::internal("Database lock failed"))?;
        crate::tool_registry::list_tools(&connection)
            .map(|tools| tools.iter().filter(|tool| tool.enabled).count() as i32)
            .unwrap_or(0)
    };
    Ok(Response::new(FooterData {
        model,
        session_id,
        // Token/cost accounting is not yet persisted per-session; the client
        // renders `—` for unknowns (prime footer contract).
        tokens_in: 0,
        tokens_out: 0,
        cost_usd: 0.0,
        tool_count,
        daemon_state: "online".to_owned(),
    }))
}
