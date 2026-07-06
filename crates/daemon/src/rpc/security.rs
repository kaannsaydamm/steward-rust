use crate::MySteward;
use steward_core::pb::{GetSecuritySettingsRequest, SecuritySettingsInfo};
use steward_core::security_settings::SecuritySettings;
use tonic::{Request, Response, Status};

pub async fn get(
    steward: &MySteward,
    _request: Request<GetSecuritySettingsRequest>,
) -> Result<Response<SecuritySettingsInfo>, Status> {
    let settings = SecuritySettings::load(&steward.security_path).map_err(internal)?;
    Ok(Response::new(SecuritySettingsInfo {
        process_exec_allowlist: settings.process_exec_allowlist,
    }))
}

pub async fn save(
    steward: &MySteward,
    request: Request<SecuritySettingsInfo>,
) -> Result<Response<SecuritySettingsInfo>, Status> {
    let request = request.into_inner();
    let settings = SecuritySettings {
        process_exec_allowlist: request.process_exec_allowlist,
    };
    settings.save(&steward.security_path).map_err(internal)?;
    Ok(Response::new(SecuritySettingsInfo {
        process_exec_allowlist: settings.process_exec_allowlist,
    }))
}

fn internal(error: anyhow::Error) -> Status {
    Status::internal(format!("{error:#}"))
}
