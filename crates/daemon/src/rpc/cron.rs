use crate::cron_jobs::{self, CronJob};
use crate::MySteward;
use steward_core::pb::{
    CreateCronJobRequest, CronJobInfo, DeleteCronJobRequest, DeleteCronJobResponse,
    ListCronJobsRequest, ListCronJobsResponse, RunCronJobNowRequest, SetCronJobEnabledRequest,
};
use tonic::{Request, Response, Status};

pub async fn list(
    steward: &MySteward,
    _request: Request<ListCronJobsRequest>,
) -> Result<Response<ListCronJobsResponse>, Status> {
    let connection = steward
        .db
        .lock()
        .map_err(|_| Status::internal("Database lock failed"))?;
    let jobs = cron_jobs::list_jobs(&connection)
        .map_err(|error| Status::internal(error.to_string()))?
        .into_iter()
        .map(cron_job_info)
        .collect();
    Ok(Response::new(ListCronJobsResponse { jobs }))
}

pub async fn create(
    steward: &MySteward,
    request: Request<CreateCronJobRequest>,
) -> Result<Response<CronJobInfo>, Status> {
    let request = request.into_inner();
    let connection = steward
        .db
        .lock()
        .map_err(|_| Status::internal("Database lock failed"))?;
    let job = cron_jobs::create_job(
        &connection,
        &request.name,
        &request.tool_id,
        &request.input_json,
        request.interval_seconds,
    )
    .map_err(|error| Status::invalid_argument(error.to_string()))?;
    Ok(Response::new(cron_job_info(job)))
}

pub async fn set_enabled(
    steward: &MySteward,
    request: Request<SetCronJobEnabledRequest>,
) -> Result<Response<CronJobInfo>, Status> {
    let request = request.into_inner();
    let connection = steward
        .db
        .lock()
        .map_err(|_| Status::internal("Database lock failed"))?;
    let job = cron_jobs::set_enabled(&connection, &request.job_id, request.enabled)
        .map_err(|error| Status::invalid_argument(error.to_string()))?;
    Ok(Response::new(cron_job_info(job)))
}

pub async fn delete(
    steward: &MySteward,
    request: Request<DeleteCronJobRequest>,
) -> Result<Response<DeleteCronJobResponse>, Status> {
    let job_id = request.into_inner().job_id;
    let connection = steward
        .db
        .lock()
        .map_err(|_| Status::internal("Database lock failed"))?;
    let deleted = cron_jobs::delete_job(&connection, &job_id)
        .map_err(|error| Status::internal(error.to_string()))?;
    Ok(Response::new(DeleteCronJobResponse { deleted }))
}

pub async fn run_now(
    steward: &MySteward,
    request: Request<RunCronJobNowRequest>,
) -> Result<Response<CronJobInfo>, Status> {
    let job_id = request.into_inner().job_id;
    let job = {
        let connection = steward
            .db
            .lock()
            .map_err(|_| Status::internal("Database lock failed"))?;
        cron_jobs::get_job(&connection, &job_id)
            .map_err(|error| Status::internal(error.to_string()))?
            .ok_or_else(|| Status::not_found(format!("cron job '{job_id}' not found")))?
    };
    let status = run_job_now(steward, &job)
        .await
        .map_err(|error| Status::internal(error.to_string()))?;
    let connection = steward
        .db
        .lock()
        .map_err(|_| Status::internal("Database lock failed"))?;
    cron_jobs::mark_run(&connection, &job.job_id, &status)
        .map_err(|error| Status::internal(error.to_string()))?;
    let refreshed = cron_jobs::get_job(&connection, &job.job_id)
        .map_err(|error| Status::internal(error.to_string()))?
        .ok_or_else(|| Status::not_found(format!("cron job '{job_id}' disappeared")))?;
    Ok(Response::new(cron_job_info(refreshed)))
}

async fn run_job_now(steward: &MySteward, job: &CronJob) -> anyhow::Result<String> {
    let arguments = serde_json::from_str::<serde_json::Value>(&job.input_json)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .map(|map| {
            map.into_iter()
                .map(|(key, value)| {
                    let value = match value {
                        serde_json::Value::String(text) => text,
                        other => other.to_string(),
                    };
                    (key, value)
                })
                .collect()
        })
        .unwrap_or_default();
    let working_directory = steward_core::storage::root()
        .map(|root| root.to_string_lossy().into_owned())
        .unwrap_or_default();
    let outcome =
        crate::tool_invocation::invoke(steward, &job.tool_id, arguments, true, &working_directory)
            .await?;
    Ok(match outcome {
        Some(result) => format!("{:?}", result.status),
        None => "tool not found".to_owned(),
    })
}

fn cron_job_info(job: CronJob) -> CronJobInfo {
    CronJobInfo {
        job_id: job.job_id,
        name: job.name,
        tool_id: job.tool_id,
        input_json: job.input_json,
        interval_seconds: job.interval_seconds,
        enabled: job.enabled,
        last_run_at: job.last_run_at,
        last_status: job.last_status,
        created_at: job.created_at,
    }
}
