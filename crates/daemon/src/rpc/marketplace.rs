use crate::artifacts::{self, ArtifactKind};
use crate::marketplace_client::{self, CLAWHUB_SKILLS_URL, SMITHERY_REGISTRY_URL};
use crate::rpc::artifacts::artifact_info;
use crate::MySteward;
use steward_core::pb::{
    ArtifactInfo, ConnectorMarketplaceEntry, InstallSkillMarketplaceEntryRequest,
    SearchConnectorMarketplaceRequest, SearchConnectorMarketplaceResponse,
    SearchSkillMarketplaceRequest, SearchSkillMarketplaceResponse, SkillMarketplaceEntry,
};
use tonic::{Request, Response, Status};

pub async fn search_connectors(
    steward: &MySteward,
    request: Request<SearchConnectorMarketplaceRequest>,
) -> Result<Response<SearchConnectorMarketplaceResponse>, Status> {
    let query = request.into_inner().query;
    match marketplace_client::search_connectors(&steward.http, SMITHERY_REGISTRY_URL, &query).await
    {
        Ok(entries) => Ok(Response::new(SearchConnectorMarketplaceResponse {
            entries: entries.into_iter().map(connector_info).collect(),
            error: String::new(),
        })),
        Err(error) => Ok(Response::new(SearchConnectorMarketplaceResponse {
            entries: Vec::new(),
            error: format!("{error:#}"),
        })),
    }
}

pub async fn search_skills(
    steward: &MySteward,
    request: Request<SearchSkillMarketplaceRequest>,
) -> Result<Response<SearchSkillMarketplaceResponse>, Status> {
    let query = request.into_inner().query;
    match marketplace_client::search_skills(&steward.http, CLAWHUB_SKILLS_URL, &query).await {
        Ok(entries) => Ok(Response::new(SearchSkillMarketplaceResponse {
            entries: entries.into_iter().map(skill_info).collect(),
            error: String::new(),
        })),
        Err(error) => Ok(Response::new(SearchSkillMarketplaceResponse {
            entries: Vec::new(),
            error: format!("{error:#}"),
        })),
    }
}

pub async fn install_skill(
    steward: &MySteward,
    request: Request<InstallSkillMarketplaceEntryRequest>,
) -> Result<Response<ArtifactInfo>, Status> {
    let slug = request.into_inner().slug;
    let (display_name, markdown) =
        marketplace_client::fetch_skill_markdown(&steward.http, CLAWHUB_SKILLS_URL, &slug)
            .await
            .map_err(|error| Status::invalid_argument(format!("{error:#}")))?;
    let connection = steward
        .db
        .lock()
        .map_err(|_| Status::internal("Database lock failed"))?;
    let artifact = artifacts::create(
        &connection,
        &format!("Skill: {display_name}"),
        ArtifactKind::Markdown,
        &markdown,
        "markdown",
        "",
    )
    .map_err(|error| Status::internal(format!("{error:#}")))?;
    Ok(Response::new(artifact_info(artifact)))
}

fn connector_info(entry: marketplace_client::ConnectorEntry) -> ConnectorMarketplaceEntry {
    ConnectorMarketplaceEntry {
        qualified_name: entry.qualified_name,
        display_name: entry.display_name,
        description: entry.description,
        homepage: entry.homepage,
        verified: entry.verified,
        use_count: entry.use_count,
        remote: entry.remote,
    }
}

fn skill_info(entry: marketplace_client::SkillEntry) -> SkillMarketplaceEntry {
    SkillMarketplaceEntry {
        slug: entry.slug,
        display_name: entry.display_name,
        summary: entry.summary,
        topics: entry.topics,
        downloads: entry.downloads,
        stars: entry.stars,
    }
}
