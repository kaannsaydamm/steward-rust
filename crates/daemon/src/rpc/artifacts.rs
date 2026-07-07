use crate::artifacts::{self, Artifact, ArtifactKind};
use crate::MySteward;
use steward_core::pb::{
    ArtifactInfo, CreateArtifactRequest, DeleteArtifactRequest, DeleteArtifactResponse,
    GetArtifactRequest, ListArtifactsRequest, ListArtifactsResponse,
};
use tonic::{Request, Response, Status};

pub async fn create(
    steward: &MySteward,
    request: Request<CreateArtifactRequest>,
) -> Result<Response<ArtifactInfo>, Status> {
    let request = request.into_inner();
    let kind = kind_from_code(request.kind).map_err(invalid)?;
    let connection = steward
        .db
        .lock()
        .map_err(|_| Status::internal("Database lock failed"))?;
    let artifact = artifacts::create(
        &connection,
        &request.title,
        kind,
        &request.content,
        &request.language,
        &request.session_id,
    )
    .map_err(invalid)?;
    Ok(Response::new(artifact_info(artifact)))
}

pub async fn list(
    steward: &MySteward,
    request: Request<ListArtifactsRequest>,
) -> Result<Response<ListArtifactsResponse>, Status> {
    let query = request.into_inner().query;
    let connection = steward
        .db
        .lock()
        .map_err(|_| Status::internal("Database lock failed"))?;
    let artifacts = artifacts::list(&connection, &query)
        .map_err(internal)?
        .into_iter()
        .map(artifact_info)
        .collect();
    Ok(Response::new(ListArtifactsResponse { artifacts }))
}

pub async fn get(
    steward: &MySteward,
    request: Request<GetArtifactRequest>,
) -> Result<Response<ArtifactInfo>, Status> {
    let artifact_id = request.into_inner().artifact_id;
    let connection = steward
        .db
        .lock()
        .map_err(|_| Status::internal("Database lock failed"))?;
    let artifact = artifacts::get(&connection, &artifact_id)
        .map_err(internal)?
        .ok_or_else(|| Status::not_found(format!("artifact '{artifact_id}' not found")))?;
    Ok(Response::new(artifact_info(artifact)))
}

pub async fn delete(
    steward: &MySteward,
    request: Request<DeleteArtifactRequest>,
) -> Result<Response<DeleteArtifactResponse>, Status> {
    let artifact_id = request.into_inner().artifact_id;
    let connection = steward
        .db
        .lock()
        .map_err(|_| Status::internal("Database lock failed"))?;
    let deleted = artifacts::delete(&connection, &artifact_id).map_err(internal)?;
    Ok(Response::new(DeleteArtifactResponse { deleted }))
}

fn kind_from_code(code: i32) -> anyhow::Result<ArtifactKind> {
    match steward_core::pb::ArtifactKind::try_from(code) {
        Ok(steward_core::pb::ArtifactKind::Code) => Ok(ArtifactKind::Code),
        Ok(steward_core::pb::ArtifactKind::Text) => Ok(ArtifactKind::Text),
        Ok(steward_core::pb::ArtifactKind::Markdown) => Ok(ArtifactKind::Markdown),
        Ok(steward_core::pb::ArtifactKind::Image) => Ok(ArtifactKind::Image),
        Ok(steward_core::pb::ArtifactKind::Link) => Ok(ArtifactKind::Link),
        Ok(steward_core::pb::ArtifactKind::Diff) => Ok(ArtifactKind::Diff),
        _ => anyhow::bail!("artifact kind is required"),
    }
}

fn kind_code(kind: ArtifactKind) -> i32 {
    match kind {
        ArtifactKind::Code => steward_core::pb::ArtifactKind::Code as i32,
        ArtifactKind::Text => steward_core::pb::ArtifactKind::Text as i32,
        ArtifactKind::Markdown => steward_core::pb::ArtifactKind::Markdown as i32,
        ArtifactKind::Image => steward_core::pb::ArtifactKind::Image as i32,
        ArtifactKind::Link => steward_core::pb::ArtifactKind::Link as i32,
        ArtifactKind::Diff => steward_core::pb::ArtifactKind::Diff as i32,
    }
}

fn artifact_info(artifact: Artifact) -> ArtifactInfo {
    ArtifactInfo {
        artifact_id: artifact.artifact_id,
        title: artifact.title,
        kind: kind_code(artifact.kind),
        content: artifact.content,
        language: artifact.language,
        session_id: artifact.session_id,
        created_at: artifact.created_at,
        updated_at: artifact.updated_at,
    }
}

fn invalid(error: anyhow::Error) -> Status {
    Status::invalid_argument(format!("{error:#}"))
}

fn internal(error: anyhow::Error) -> Status {
    Status::internal(format!("{error:#}"))
}
