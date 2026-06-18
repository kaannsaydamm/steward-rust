use crate::ui::HistoryLine;
use crate::{client, registry_view};
use anyhow::Result;

pub async fn tool_lines(host: &str) -> Result<Vec<HistoryLine>> {
    Ok(client::list_tools(host)
        .await?
        .iter()
        .map(registry_view::tool_line)
        .map(HistoryLine::agent)
        .collect())
}

pub async fn skill_lines(host: &str) -> Result<Vec<HistoryLine>> {
    Ok(client::list_skills(host)
        .await?
        .iter()
        .map(registry_view::skill_line)
        .map(HistoryLine::agent)
        .collect())
}
