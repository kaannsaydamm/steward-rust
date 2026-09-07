//! Agent profile builder RPCs: persist, validate, import/export YAML profiles.
//!
//! Ported from microsoft/autogen@027ecf0a379bcc1d09956d46d12d44a3ad9cee14
//! autogen-studio builder semantics (MIT). Modified for Steward: profiles are
//! `harness::AgentProfile` documents (YAML) under `~/.steward/agents/`;
//! `validate()` is enforced on every write.

use crate::MySteward;
use anyhow::{Context as _, Result};
use steward_core::pb::{
    AgentPersona, AgentProfileInfo, DeleteAgentProfileRequest, DeleteAgentProfileResponse,
    ExportAgentProfileRequest, ExportAgentProfileResponse, HookRef, ImportAgentProfileRequest,
    ListAgentProfilesRequest, ListAgentProfilesResponse, SaveAgentProfileRequest,
};
use steward_harness::agent_profile::AgentProfile;
use tonic::{Request, Response, Status};

fn agents_dir(steward: &MySteward) -> Result<std::path::PathBuf> {
    let root = steward
        .provider_path
        .parent()
        .context("steward data root has no parent")?
        .join("agents");
    std::fs::create_dir_all(&root)?;
    Ok(root)
}

fn profile_to_info(id: &str, yaml: &str) -> AgentProfileInfo {
    match AgentProfile::parse_yaml(yaml) {
        Ok(profile) => AgentProfileInfo {
            id: profile.id.clone(),
            display_name: profile.title.clone(),
            persona: Some(AgentPersona {
                role: profile.title.clone(),
                system_instructions: profile.description.clone(),
            }),
            hooks: Vec::new(),
            skills: profile.context.inherit.clone(),
            subagents: Vec::new(),
            template: false,
            model_policy: format!("{:?}", profile.model).to_lowercase(),
            explicit_profile_id: String::new(),
            yaml: yaml.to_owned(),
            error: String::new(),
        },
        Err(error) => AgentProfileInfo {
            id: id.to_owned(),
            display_name: String::new(),
            persona: None,
            hooks: Vec::new(),
            skills: Vec::new(),
            subagents: Vec::new(),
            template: false,
            model_policy: String::new(),
            explicit_profile_id: String::new(),
            yaml: yaml.to_owned(),
            error: format!("{error}"),
        },
    }
}

fn parse_request_yaml(yaml: &str) -> std::result::Result<AgentProfile, Status> {
    let profile = AgentProfile::parse_yaml(yaml)
        .map_err(|error| Status::invalid_argument(format!("invalid profile YAML: {error}")))?;
    profile
        .validate()
        .map_err(|error| Status::invalid_argument(format!("{error}")))?;
    Ok(profile)
}

fn write_profile_file(steward: &MySteward, id: &str, yaml: &str) -> Result<std::path::PathBuf> {
    if id.contains('/') || id.contains('\\') || id.contains("..") {
        anyhow::bail!("profile id '{id}' is not a safe file name");
    }
    let path = agents_dir(steward)?.join(format!("{id}.yaml"));
    std::fs::write(&path, yaml)?;
    Ok(path)
}

pub async fn save(
    steward: &MySteward,
    request: Request<SaveAgentProfileRequest>,
) -> Result<Response<AgentProfileInfo>, Status> {
    let yaml = request.into_inner().yaml;
    let profile = parse_request_yaml(&yaml)?;
    write_profile_file(steward, &profile.id, &yaml)
        .map_err(|error| Status::internal(format!("{error:#}")))?;
    Ok(Response::new(profile_to_info(&profile.id, &yaml)))
}

pub async fn import(
    steward: &MySteward,
    request: Request<ImportAgentProfileRequest>,
) -> Result<Response<AgentProfileInfo>, Status> {
    // Import and save share validation; import is the file-oriented entry.
    save(
        steward,
        Request::new(SaveAgentProfileRequest {
            yaml: request.into_inner().yaml,
        }),
    )
    .await
}

pub async fn delete(
    steward: &MySteward,
    request: Request<DeleteAgentProfileRequest>,
) -> Result<Response<DeleteAgentProfileResponse>, Status> {
    let id = request.into_inner().id;
    if id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err(Status::invalid_argument(format!(
            "profile id '{id}' is not a safe file name"
        )));
    }
    let path = agents_dir(steward)
        .map_err(|error| Status::internal(format!("{error:#}")))?
        .join(format!("{id}.yaml"));
    let deleted = path.exists() && std::fs::remove_file(&path).is_ok();
    Ok(Response::new(DeleteAgentProfileResponse { deleted }))
}

pub async fn list(
    steward: &MySteward,
    _request: Request<ListAgentProfilesRequest>,
) -> Result<Response<ListAgentProfilesResponse>, Status> {
    let dir = agents_dir(steward).map_err(|error| Status::internal(format!("{error:#}")))?;
    let mut profiles = Vec::new();
    let entries =
        std::fs::read_dir(&dir).map_err(|error| Status::internal(format!("{error:#}")))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("yaml") {
            continue;
        }
        let yaml = match std::fs::read_to_string(&path) {
            Ok(yaml) => yaml,
            Err(_) => continue,
        };
        let id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .to_owned();
        profiles.push(profile_to_info(&id, &yaml));
    }
    profiles.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Response::new(ListAgentProfilesResponse { profiles }))
}

pub async fn export(
    steward: &MySteward,
    request: Request<ExportAgentProfileRequest>,
) -> Result<Response<ExportAgentProfileResponse>, Status> {
    let id = request.into_inner().id;
    if id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err(Status::invalid_argument(format!(
            "profile id '{id}' is not a safe file name"
        )));
    }
    let path = agents_dir(steward)
        .map_err(|error| Status::internal(format!("{error:#}")))?
        .join(format!("{id}.yaml"));
    let yaml = std::fs::read_to_string(&path)
        .map_err(|_| Status::not_found(format!("agent profile '{id}' not found")))?;
    Ok(Response::new(ExportAgentProfileResponse { yaml }))
}
