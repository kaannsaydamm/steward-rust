use crate::mcp_registry::{self, AdapterConfig};
use crate::mcp_runtime::{AdapterState, RuntimeStatus};
use crate::MySteward;
use anyhow::{bail, Result};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct AdapterInfo {
    pub config: AdapterConfig,
    pub status: RuntimeStatus,
}

pub async fn register(steward: &MySteward, config: AdapterConfig) -> Result<AdapterInfo> {
    if steward.mcp_runtime.status(&config.id).await.state == AdapterState::Running {
        bail!(
            "stop MCP adapter '{}' before changing its configuration",
            config.id
        );
    }
    steward.mcp_runtime.stop(&config.id).await?;
    {
        let connection = steward
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
        mcp_registry::register(&connection, &config)?;
    }
    Ok(AdapterInfo {
        config,
        status: RuntimeStatus {
            state: AdapterState::Stopped,
            server_name: String::new(),
            server_version: String::new(),
            tool_count: 0,
            error: String::new(),
        },
    })
}

pub async fn list(steward: &MySteward) -> Result<Vec<AdapterInfo>> {
    let configs = {
        let connection = steward
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
        mcp_registry::list(&connection)?
    };
    let mut adapters = Vec::with_capacity(configs.len());
    for config in configs {
        let status = steward.mcp_runtime.status(&config.id).await;
        if status.state != AdapterState::Running {
            let connection = steward
                .db
                .lock()
                .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
            mcp_registry::set_tools_enabled(&connection, &config.id, false)?;
        }
        adapters.push(AdapterInfo { config, status });
    }
    Ok(adapters)
}

pub async fn start(steward: &MySteward, adapter_id: &str) -> Result<AdapterInfo> {
    let config = load_config(steward, adapter_id)?;
    {
        let connection = steward
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
        mcp_registry::set_tools_enabled(&connection, adapter_id, false)?;
    }
    let started = steward.mcp_runtime.start(&config).await?;
    let tool_count = started.tools.len();
    let registry_result = {
        let connection = steward
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
        mcp_registry::replace_tools(&connection, adapter_id, &started.tools)
    };
    if let Err(error) = registry_result {
        steward.mcp_runtime.stop(adapter_id).await?;
        return Err(error);
    }
    Ok(AdapterInfo {
        config,
        status: RuntimeStatus {
            state: AdapterState::Running,
            server_name: started.identity.name,
            server_version: started.identity.version,
            tool_count,
            error: String::new(),
        },
    })
}

pub async fn stop(steward: &MySteward, adapter_id: &str) -> Result<AdapterInfo> {
    let config = load_config(steward, adapter_id)?;
    steward.mcp_runtime.stop(adapter_id).await?;
    let connection = steward
        .db
        .lock()
        .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
    mcp_registry::set_tools_enabled(&connection, adapter_id, false)?;
    Ok(AdapterInfo {
        config,
        status: RuntimeStatus {
            state: AdapterState::Stopped,
            server_name: String::new(),
            server_version: String::new(),
            tool_count: 0,
            error: String::new(),
        },
    })
}

pub async fn remove(steward: &MySteward, adapter_id: &str) -> Result<bool> {
    steward.mcp_runtime.stop(adapter_id).await?;
    let connection = steward
        .db
        .lock()
        .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
    mcp_registry::remove(&connection, adapter_id)
}

pub async fn invoke(
    steward: &MySteward,
    tool_id: &str,
    arguments: &BTreeMap<String, String>,
) -> Result<String> {
    let binding = {
        let connection = steward
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
        mcp_registry::binding(&connection, tool_id)?
    };
    let binding = binding.ok_or_else(|| anyhow::anyhow!("MCP tool binding not found"))?;
    steward
        .mcp_runtime
        .call(&binding.adapter_id, &binding.remote_name, arguments)
        .await
}

fn load_config(steward: &MySteward, adapter_id: &str) -> Result<AdapterConfig> {
    let connection = steward
        .db
        .lock()
        .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
    mcp_registry::get(&connection, adapter_id)?
        .ok_or_else(|| anyhow::anyhow!("MCP adapter '{adapter_id}' not found"))
}

#[cfg(test)]
#[path = "mcp_lifecycle_tests.rs"]
mod tests;
