use crate::client;
use anyhow::Result;

pub struct OperatorStatus {
    pub daemon: String,
    pub agents: usize,
    pub workflows: usize,
    pub memories: usize,
    pub dreams: usize,
}

impl OperatorStatus {
    pub fn lines(&self) -> Vec<String> {
        vec![
            format!("daemon: {}", self.daemon),
            format!("agents: {}", self.agents),
            format!("workflows: {}", self.workflows),
            format!("memories: {}", self.memories),
            format!("dreams: {}", self.dreams),
        ]
    }
}

pub async fn load(host: &str) -> Result<OperatorStatus> {
    let daemon = client::ping(host).await?;
    let agents = client::list_agents(host).await?.len();
    let workflows = client::list_workflows(host).await?.len();
    let memories = client::recall_memory(host, "", 0, 25).await?.len();
    let dreams = client::recall_memory(host, "", 4, 25).await?.len();
    Ok(OperatorStatus {
        daemon,
        agents,
        workflows,
        memories,
        dreams,
    })
}
