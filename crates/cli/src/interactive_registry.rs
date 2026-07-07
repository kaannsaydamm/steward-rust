use crate::ui::HistoryLine;
use crate::{client, registry_commands, registry_view};
use anyhow::Result;
use std::path::Path;

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

pub async fn install_skill_lines(host: &str, raw_path: &str) -> Result<Vec<HistoryLine>> {
    let path = Path::new(raw_path.trim());
    let bundle = std::fs::read(path)?;
    let skill = client::install_skill(host, bundle).await?;
    Ok(vec![HistoryLine::system(format!(
        "installed {} version={}",
        skill.skill_id, skill.version
    ))])
}

pub async fn skill_marketplace_lines(host: &str, query: &str) -> Result<Vec<HistoryLine>> {
    let (entries, error) = client::search_skill_marketplace(host, query).await?;
    if !error.is_empty() {
        return Ok(vec![HistoryLine::error(error)]);
    }
    Ok(entries
        .iter()
        .map(|entry| {
            HistoryLine::agent(format!(
                "{}\tdownloads={}\tstars={}\t[{}]\t{}",
                entry.slug,
                entry.downloads,
                entry.stars,
                entry.topics.join(","),
                entry.summary.replace('\n', " ")
            ))
        })
        .collect())
}

pub async fn install_skill_marketplace_line(host: &str, slug: &str) -> Result<HistoryLine> {
    let artifact = client::install_skill_marketplace_entry(host, slug).await?;
    Ok(HistoryLine::system(format!(
        "saved as artifact\t{}\t{}",
        artifact.artifact_id, artifact.title
    )))
}

pub async fn invoke_lines(host: &str, raw: &str) -> Result<Vec<HistoryLine>> {
    let mut parts = raw.split_whitespace();
    let Some(tool_id) = parts.next() else {
        return Ok(vec![HistoryLine::error(
            "usage: /invoke <tool_id> [--approve] [key=value ...]".to_owned(),
        )]);
    };
    let mut approved = false;
    let mut raw_arguments = Vec::new();
    for part in parts {
        if part == "--approve" {
            approved = true;
        } else {
            raw_arguments.push(part.to_owned());
        }
    }
    let arguments = registry_commands::parse_arguments(&raw_arguments)?;
    let response = client::invoke_tool(host, tool_id, arguments, approved).await?;
    let mut lines = vec![HistoryLine::system(
        registry_view::invocation_response_line(&response),
    )];
    if !response.output.is_empty() {
        lines.extend(
            response
                .output
                .lines()
                .map(|line| HistoryLine::agent(line.to_owned())),
        );
    }
    Ok(lines)
}

pub async fn history_lines(host: &str) -> Result<Vec<HistoryLine>> {
    Ok(client::list_tool_invocations(host, 20)
        .await?
        .iter()
        .map(registry_view::invocation_line)
        .map(HistoryLine::agent)
        .collect())
}
