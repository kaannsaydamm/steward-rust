use crate::mcp_registry::AdapterConfig;
use crate::mcp_session::{McpSession, StartedSession};
use anyhow::{bail, Result};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdapterState {
    Stopped,
    Running,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStatus {
    pub state: AdapterState,
    pub server_name: String,
    pub server_version: String,
    pub tool_count: usize,
    pub error: String,
}

#[derive(Clone)]
enum RuntimeEntry {
    Running(Arc<Mutex<McpSession>>),
    Failed(String),
}

pub struct McpRuntime {
    entries: Mutex<HashMap<String, RuntimeEntry>>,
}

impl McpRuntime {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    pub async fn start(&self, config: &AdapterConfig) -> Result<StartedSession> {
        self.stop(&config.id).await?;
        match McpSession::connect(config).await {
            Ok((session, started)) => {
                self.entries.lock().await.insert(
                    config.id.clone(),
                    RuntimeEntry::Running(Arc::new(Mutex::new(session))),
                );
                Ok(started)
            }
            Err(error) => {
                self.entries.lock().await.insert(
                    config.id.clone(),
                    RuntimeEntry::Failed(format!("{error:#}")),
                );
                Err(error)
            }
        }
    }

    pub async fn stop(&self, adapter_id: &str) -> Result<()> {
        let entry = self.entries.lock().await.remove(adapter_id);
        if let Some(RuntimeEntry::Running(session)) = entry {
            session.lock().await.shutdown().await?;
        }
        Ok(())
    }

    pub async fn call(
        &self,
        adapter_id: &str,
        remote_name: &str,
        arguments: &BTreeMap<String, String>,
    ) -> Result<String> {
        let entry = self.entries.lock().await.get(adapter_id).cloned();
        let Some(RuntimeEntry::Running(session)) = entry else {
            bail!("MCP adapter '{adapter_id}' is not running");
        };
        let mut session = session.lock().await;
        session.call_tool(remote_name, arguments).await
    }

    pub async fn status(&self, adapter_id: &str) -> RuntimeStatus {
        let entry = self.entries.lock().await.get(adapter_id).cloned();
        match entry {
            Some(RuntimeEntry::Running(session)) => {
                let mut session = session.lock().await;
                match session.child.try_wait() {
                    Ok(Some(exit)) => RuntimeStatus::failed(format!("process exited with {exit}")),
                    Ok(None) => RuntimeStatus {
                        state: AdapterState::Running,
                        server_name: session.identity.name.clone(),
                        server_version: session.identity.version.clone(),
                        tool_count: session.tool_count,
                        error: String::new(),
                    },
                    Err(error) => RuntimeStatus::failed(error.to_string()),
                }
            }
            Some(RuntimeEntry::Failed(error)) => RuntimeStatus::failed(error),
            None => RuntimeStatus::stopped(),
        }
    }
}

impl RuntimeStatus {
    fn stopped() -> Self {
        Self {
            state: AdapterState::Stopped,
            server_name: String::new(),
            server_version: String::new(),
            tool_count: 0,
            error: String::new(),
        }
    }

    fn failed(error: String) -> Self {
        Self {
            state: AdapterState::Failed,
            error,
            ..Self::stopped()
        }
    }
}
