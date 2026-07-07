use crate::provider_client;
use crate::MySteward;
use steward_core::pb::{
    ActivateProviderProfileRequest, DeleteProviderProfileRequest, DeleteProviderProfileResponse,
    ListProviderCatalogRequest, ListProviderCatalogResponse, ListProviderModelsRequest,
    ListProviderModelsResponse, ListProviderProfilesRequest, ListProviderProfilesResponse,
    ProviderCatalogEntry, ProviderProfileInfo, SaveProviderProfileRequest,
};
use steward_core::provider_catalog;
use steward_core::provider_config::{ProviderProfile, ProviderProtocol, ProviderSettings};
use tonic::{Request, Response, Status};

pub async fn catalog(
    _request: Request<ListProviderCatalogRequest>,
) -> Result<Response<ListProviderCatalogResponse>, Status> {
    let providers = provider_catalog::PROVIDERS
        .iter()
        .map(|provider| ProviderCatalogEntry {
            provider_id: provider.id.to_owned(),
            name: provider.name.to_owned(),
            protocol: protocol_code(provider.protocol),
            default_base_url: provider.default_base_url.to_owned(),
            default_api_key_env: provider.default_api_key_env.unwrap_or_default().to_owned(),
        })
        .collect();
    Ok(Response::new(ListProviderCatalogResponse { providers }))
}

pub async fn list(
    steward: &MySteward,
    _request: Request<ListProviderProfilesRequest>,
) -> Result<Response<ListProviderProfilesResponse>, Status> {
    let settings = load(steward).map_err(internal)?;
    let profiles = settings
        .profiles
        .iter()
        .map(|profile| profile_info(profile, settings.active_profile.as_deref()))
        .collect();
    Ok(Response::new(ListProviderProfilesResponse { profiles }))
}

pub async fn save(
    steward: &MySteward,
    request: Request<SaveProviderProfileRequest>,
) -> Result<Response<ProviderProfileInfo>, Status> {
    let request = request.into_inner();
    let info = request
        .profile
        .ok_or_else(|| Status::invalid_argument("provider profile is required"))?;
    let mut settings = load(steward).map_err(internal)?;
    let existing_key = settings
        .get(&info.profile_id)
        .and_then(|existing| existing.api_key.clone());
    let mut profile = profile_from_info(info).map_err(invalid)?;
    if profile.api_key.is_none() {
        profile.api_key = existing_key;
    }
    settings.upsert(profile.clone()).map_err(invalid)?;
    if request.activate || settings.active_profile.is_none() {
        settings.activate(&profile.profile_id).map_err(invalid)?;
    }
    settings.save(&steward.provider_path).map_err(internal)?;
    Ok(Response::new(profile_info(
        &profile,
        settings.active_profile.as_deref(),
    )))
}

pub async fn activate(
    steward: &MySteward,
    request: Request<ActivateProviderProfileRequest>,
) -> Result<Response<ProviderProfileInfo>, Status> {
    let profile_id = request.into_inner().profile_id;
    let mut settings = load(steward).map_err(internal)?;
    settings.activate(&profile_id).map_err(invalid)?;
    settings.save(&steward.provider_path).map_err(internal)?;
    let profile = settings
        .get(&profile_id)
        .ok_or_else(|| Status::internal("activated provider profile disappeared"))?;
    Ok(Response::new(profile_info(
        profile,
        settings.active_profile.as_deref(),
    )))
}

pub async fn delete(
    steward: &MySteward,
    request: Request<DeleteProviderProfileRequest>,
) -> Result<Response<DeleteProviderProfileResponse>, Status> {
    let profile_id = request.into_inner().profile_id;
    let mut settings = load(steward).map_err(internal)?;
    settings.remove(&profile_id).map_err(invalid)?;
    settings.save(&steward.provider_path).map_err(internal)?;
    Ok(Response::new(DeleteProviderProfileResponse {
        deleted: true,
    }))
}

pub async fn list_models(
    steward: &MySteward,
    request: Request<ListProviderModelsRequest>,
) -> Result<Response<ListProviderModelsResponse>, Status> {
    let request = request.into_inner();
    let protocol = match steward_core::pb::ProviderProtocol::try_from(request.protocol) {
        Ok(steward_core::pb::ProviderProtocol::OpenaiChat) => ProviderProtocol::OpenAiChat,
        Ok(steward_core::pb::ProviderProtocol::AnthropicMessages) => {
            ProviderProtocol::AnthropicMessages
        }
        Ok(steward_core::pb::ProviderProtocol::GeminiGenerateContent) => {
            ProviderProtocol::GeminiGenerateContent
        }
        _ => {
            return Ok(Response::new(ListProviderModelsResponse {
                model_ids: Vec::new(),
                error: "provider protocol is required".to_owned(),
            }))
        }
    };
    let api_key = if !request.api_key.trim().is_empty() {
        Some(request.api_key.trim().to_owned())
    } else if request.api_key_env.trim().is_empty() {
        None
    } else {
        match std::env::var(request.api_key_env.trim()) {
            Ok(value) => Some(value),
            Err(_) => {
                return Ok(Response::new(ListProviderModelsResponse {
                    model_ids: Vec::new(),
                    error: format!(
                        "environment variable {} is not set",
                        request.api_key_env.trim()
                    ),
                }))
            }
        }
    };
    match provider_client::list_models(
        &steward.http,
        protocol,
        &request.base_url,
        api_key.as_deref(),
    )
    .await
    {
        Ok(model_ids) => Ok(Response::new(ListProviderModelsResponse {
            model_ids,
            error: String::new(),
        })),
        Err(error) => Ok(Response::new(ListProviderModelsResponse {
            model_ids: Vec::new(),
            error: error.to_string(),
        })),
    }
}

fn load(steward: &MySteward) -> anyhow::Result<ProviderSettings> {
    ProviderSettings::load(&steward.provider_path)
}

fn profile_from_info(value: ProviderProfileInfo) -> anyhow::Result<ProviderProfile> {
    let protocol = match steward_core::pb::ProviderProtocol::try_from(value.protocol)
        .map_err(|_| anyhow::anyhow!("provider protocol is invalid"))?
    {
        steward_core::pb::ProviderProtocol::OpenaiChat => ProviderProtocol::OpenAiChat,
        steward_core::pb::ProviderProtocol::AnthropicMessages => {
            ProviderProtocol::AnthropicMessages
        }
        steward_core::pb::ProviderProtocol::GeminiGenerateContent => {
            ProviderProtocol::GeminiGenerateContent
        }
        steward_core::pb::ProviderProtocol::Unspecified => {
            anyhow::bail!("provider protocol is required");
        }
    };
    Ok(ProviderProfile {
        profile_id: value.profile_id,
        provider_id: value.provider_id,
        display_name: value.display_name,
        protocol,
        base_url: value.base_url,
        model: value.model,
        api_key_env: (!value.api_key_env.is_empty()).then_some(value.api_key_env),
        api_key: (!value.api_key.is_empty()).then_some(value.api_key),
    })
}

fn profile_info(profile: &ProviderProfile, active: Option<&str>) -> ProviderProfileInfo {
    ProviderProfileInfo {
        profile_id: profile.profile_id.clone(),
        provider_id: profile.provider_id.clone(),
        display_name: profile.display_name.clone(),
        protocol: protocol_code(profile.protocol),
        base_url: profile.base_url.clone(),
        model: profile.model.clone(),
        api_key_env: profile.api_key_env.clone().unwrap_or_default(),
        active: active == Some(profile.profile_id.as_str()),
        api_key: String::new(),
        has_stored_key: profile.api_key.is_some(),
    }
}

const fn protocol_code(protocol: ProviderProtocol) -> i32 {
    match protocol {
        ProviderProtocol::OpenAiChat => steward_core::pb::ProviderProtocol::OpenaiChat as i32,
        ProviderProtocol::AnthropicMessages => {
            steward_core::pb::ProviderProtocol::AnthropicMessages as i32
        }
        ProviderProtocol::GeminiGenerateContent => {
            steward_core::pb::ProviderProtocol::GeminiGenerateContent as i32
        }
    }
}

fn invalid(error: anyhow::Error) -> Status {
    Status::invalid_argument(format!("{error:#}"))
}

fn internal(error: anyhow::Error) -> Status {
    Status::internal(format!("{error:#}"))
}
