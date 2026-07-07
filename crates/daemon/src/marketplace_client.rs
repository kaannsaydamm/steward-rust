//! Thin clients for two public, unauthenticated third-party catalogs the user wants browsable
//! inside Steward's own marketplace UI: Smithery's MCP connector registry and ClawHub's Claude
//! skill registry. Both are queried live — no cached/static snapshot of their listings, since
//! both catalogs change constantly and a frozen copy would misrepresent what's actually there.

use anyhow::{Context as _, Result};
use serde::Deserialize;

pub const SMITHERY_REGISTRY_URL: &str = "https://registry.smithery.ai/servers";
pub const CLAWHUB_SKILLS_URL: &str = "https://clawhub.ai/api/v1/skills";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConnectorEntry {
    pub qualified_name: String,
    pub display_name: String,
    pub description: String,
    pub homepage: String,
    pub verified: bool,
    pub use_count: i64,
    pub remote: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkillEntry {
    pub slug: String,
    pub display_name: String,
    pub summary: String,
    pub topics: Vec<String>,
    pub downloads: i64,
    pub stars: i64,
}

#[derive(Deserialize)]
struct SmitheryResponse {
    #[serde(default)]
    servers: Vec<SmitheryServer>,
}

#[derive(Deserialize, Default)]
struct SmitheryServer {
    #[serde(rename = "qualifiedName", default)]
    qualified_name: String,
    #[serde(rename = "displayName", default)]
    display_name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    homepage: String,
    #[serde(default)]
    verified: bool,
    #[serde(rename = "useCount", default)]
    use_count: i64,
    #[serde(default = "default_true")]
    remote: bool,
}

const fn default_true() -> bool {
    true
}

pub async fn search_connectors(
    http: &reqwest::Client,
    registry_url: &str,
    query: &str,
) -> Result<Vec<ConnectorEntry>> {
    let response = http
        .get(registry_url)
        .query(&[("q", query), ("pageSize", "20")])
        .send()
        .await
        .context("querying the Smithery connector registry")?
        .error_for_status()
        .context("Smithery connector registry returned an error")?;
    let body: SmitheryResponse = response
        .json()
        .await
        .context("parsing the Smithery connector registry response")?;
    Ok(body
        .servers
        .into_iter()
        .map(|server| ConnectorEntry {
            qualified_name: server.qualified_name,
            display_name: server.display_name,
            description: server.description,
            homepage: server.homepage,
            verified: server.verified,
            use_count: server.use_count,
            remote: server.remote,
        })
        .collect())
}

#[derive(Deserialize)]
struct ClawhubSearchResponse {
    #[serde(default)]
    items: Vec<ClawhubSkillSummary>,
}

#[derive(Deserialize, Default)]
struct ClawhubSkillSummary {
    #[serde(default)]
    slug: String,
    #[serde(rename = "displayName", default)]
    display_name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    topics: Vec<String>,
    #[serde(default)]
    stats: ClawhubStats,
}

#[derive(Deserialize, Default)]
struct ClawhubStats {
    #[serde(default)]
    downloads: i64,
    #[serde(default)]
    stars: i64,
}

pub async fn search_skills(
    http: &reqwest::Client,
    skills_url: &str,
    query: &str,
) -> Result<Vec<SkillEntry>> {
    let response = http
        .get(skills_url)
        .query(&[("q", query)])
        .send()
        .await
        .context("querying the ClawHub skill registry")?
        .error_for_status()
        .context("ClawHub skill registry returned an error")?;
    let body: ClawhubSearchResponse = response
        .json()
        .await
        .context("parsing the ClawHub skill registry response")?;
    Ok(body
        .items
        .into_iter()
        .map(|skill| SkillEntry {
            slug: skill.slug,
            display_name: skill.display_name,
            summary: skill.summary,
            topics: skill.topics,
            downloads: skill.stats.downloads,
            stars: skill.stats.stars,
        })
        .collect())
}

#[derive(Deserialize)]
struct ClawhubDetailResponse {
    skill: ClawhubSkillDetail,
}

#[derive(Deserialize, Default)]
struct ClawhubSkillDetail {
    #[serde(rename = "displayName", default)]
    display_name: String,
    #[serde(default)]
    description: String,
}

/// Returns `(display_name, full SKILL.md markdown)` for one ClawHub skill.
pub async fn fetch_skill_markdown(
    http: &reqwest::Client,
    skills_url: &str,
    slug: &str,
) -> Result<(String, String)> {
    if slug.trim().is_empty() || !slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        anyhow::bail!("invalid skill slug '{slug}'");
    }
    let url = format!("{skills_url}/{slug}");
    let response = http
        .get(&url)
        .send()
        .await
        .context("fetching the ClawHub skill detail")?
        .error_for_status()
        .context("ClawHub returned an error for this skill")?;
    let body: ClawhubDetailResponse = response
        .json()
        .await
        .context("parsing the ClawHub skill detail response")?;
    let display_name = if body.skill.display_name.is_empty() {
        slug.to_owned()
    } else {
        body.skill.display_name
    };
    Ok((display_name, body.skill.description))
}

#[cfg(test)]
#[path = "marketplace_client_tests.rs"]
mod tests;
