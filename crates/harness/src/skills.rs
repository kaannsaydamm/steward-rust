//! Agent Skills (§29, Task 16.1, C-006, P-016): Agent-Skills-compatible
//! packages with progressive disclosure — metadata first, body on demand,
//! references/scripts only when needed.

use anyhow::{Context as _, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Index entry: name/description only (level 1) — cheap to load all.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Task/path triggers for lazy activation.
    #[serde(default)]
    pub triggers: Vec<String>,
    /// Provenance: source + sha256 of the bundle (P-016).
    pub provenance: SkillProvenance,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillProvenance {
    pub source: String,
    pub content_hash: String,
    pub installed_at_ms: i64,
}

/// Full body + references (level 2/3): loaded only when the skill is picked.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadedSkill {
    pub summary: SkillSummary,
    pub body: String,
    pub references: Vec<(String, String)>,
    pub scripts: Vec<String>,
}

/// The registry: level-1 index always resident; level-2/3 lazy.
pub struct SkillRegistry {
    index: RwLock<BTreeMap<String, SkillSummary>>,
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self { index: RwLock::new(BTreeMap::new()) }
    }

    /// Scans a skills directory: each subdir needs a SKILL.md with
    /// `name`/`description` frontmatter. Records provenance by hash.
    pub fn load_directory(&self, directory: &Path) -> Result<usize> {
        let mut loaded = 0;
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(_) => return Ok(0),
        };
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let skill_md = path.join("SKILL.md");
            if !path.is_dir() || !skill_md.exists() {
                continue;
            }
            let content = std::fs::read_to_string(&skill_md)?;
            let summary = parse_skill_md(&content, &entry.file_name().to_string_lossy())?;
            let provenance = SkillProvenance {
                source: path.display().to_string(),
                content_hash: hash_dir(&path),
                installed_at_ms: now_ms(),
            };
            self.index.write().insert(
                summary.id.clone(),
                SkillSummary { provenance, ..summary },
            );
            loaded += 1;
        }
        Ok(loaded)
    }

    /// Level 1: metadata index without bodies.
    pub fn summaries(&self) -> Vec<SkillSummary> {
        self.index.read().values().cloned().collect()
    }

    /// Trigger-matched summaries (progressive activation).
    pub fn match_triggers(&self, task_text: &str) -> Vec<SkillSummary> {
        let task = task_text.to_lowercase();
        self.index
            .read()
            .values()
            .filter(|summary| {
                summary
                    .triggers
                    .iter()
                    .any(|trigger| task.contains(&trigger.to_lowercase()))
            })
            .cloned()
            .collect()
    }

    /// Level 2: full body for the chosen skill.
    pub fn load_full(&self, skill_id: &str) -> Result<LoadedSkill> {
        let summary = self
            .index
            .read()
            .get(skill_id)
            .cloned()
            .context("skill not installed")?;
        let source = Path::new(&summary.provenance.source);
        let body = std::fs::read_to_string(source.join("SKILL.md"))?;
        let mut references = Vec::new();
        let references_dir = source.join("references");
        if references_dir.is_dir() {
            for entry in std::fs::read_dir(&references_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    references.push((
                        path.file_name().unwrap_or_default().to_string_lossy().to_string(),
                        std::fs::read_to_string(&path)?,
                    ));
                }
            }
        }
        let mut scripts = Vec::new();
        let scripts_dir = source.join("scripts");
        if scripts_dir.is_dir() {
            for entry in std::fs::read_dir(&scripts_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    scripts.push(path.display().to_string());
                }
            }
        }
        Ok(LoadedSkill { summary, body, references, scripts })
    }
}

/// Parses `SKILL.md` with `name`/`description` frontmatter.
fn parse_skill_md(content: &str, fallback_id: &str) -> Result<SkillSummary> {
    let rest = content
        .trim_start()
        .strip_prefix("---")
        .context("SKILL.md must start with --- frontmatter")?;
    let (frontmatter, _) = rest
        .split_once("---")
        .context("SKILL.md frontmatter must close")?;
    let mut name = String::new();
    let mut description = String::new();
    let mut triggers = Vec::new();
    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("name:") {
            name = value.trim().to_owned();
        } else if let Some(value) = line.strip_prefix("description:") {
            description = value.trim().to_owned();
        } else if let Some(value) = line.strip_prefix("triggers:") {
            triggers = value
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(|t| t.trim().trim_matches('"').to_owned())
                .filter(|t| !t.is_empty())
                .collect();
        }
    }
    let id = fallback_id.to_owned();
    Ok(SkillSummary {
        id,
        name: if name.is_empty() { fallback_id.to_owned() } else { name },
        description,
        triggers,
        provenance: SkillProvenance {
            source: String::new(),
            content_hash: String::new(),
            installed_at_ms: 0,
        },
    })
}

fn hash_dir(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    let mut files: Vec<PathBuf> = Vec::new();
    collect(path, &mut files);
    files.sort();
    for file in files {
        hasher.update(file.file_name().unwrap_or_default().as_encoded_bytes());
        if let Ok(content) = std::fs::read(&file) {
            hasher.update(&content);
        }
    }
    format!("{:x}", hasher.finalize())
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect(&path, files);
            } else {
                files.push(path);
            }
        }
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn install_fixture() -> (tempfile::TempDir, SkillRegistry) {
        let temp = tempfile::tempdir().unwrap();
        let registry = SkillRegistry::new();

        let rust_skill = temp.path().join("rust-debug");
        std::fs::create_dir_all(rust_skill.join("references")).unwrap();
        std::fs::create_dir_all(rust_skill.join("scripts")).unwrap();
        std::fs::write(
            rust_skill.join("SKILL.md"),
            "---\nname: Rust Debug\ndescription: diagnose rust compile failures\ntriggers: [\"rust\", \"cargo\", \"compile\"]\n---\nStep 1: run cargo check.",
        )
        .unwrap();
        std::fs::write(rust_skill.join("references").join("linker.md"), "linker notes").unwrap();
        std::fs::write(rust_skill.join("scripts").join("collect.ps1"), "Get-Content").unwrap();

        let git_skill = temp.path().join("git-flow");
        std::fs::create_dir_all(&git_skill).unwrap();
        std::fs::write(
            git_skill.join("SKILL.md"),
            "---\nname: Git Flow\ndescription: branch and merge hygiene\ntriggers: [\"git\", \"branch\"]\n---\nAlways rebase.",
        )
        .unwrap();

        registry.load_directory(temp.path()).unwrap();
        (temp, registry)
    }

    #[test]
    fn index_records_metadata_without_loading_bodies() {
        let (_temp, registry) = install_fixture();
        let summaries = registry.summaries();
        assert_eq!(summaries.len(), 2);
        let rust = summaries.iter().find(|s| s.id == "rust-debug").unwrap();
        assert_eq!(rust.name, "Rust Debug");
        assert!(rust.provenance.content_hash.len() == 64, "content hash recorded");
        assert!(rust.provenance.installed_at_ms > 0);
    }

    #[test]
    fn triggers_match_tasks_progressively() {
        let (_temp, registry) = install_fixture();
        let hits = registry.match_triggers("this cargo build fails with rust errors");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "rust-debug");
        // No trigger match → no skill loads.
        assert!(registry.match_triggers("write a poem").is_empty());
    }

    #[test]
    fn full_load_pulls_body_references_scripts() {
        let (_temp, registry) = install_fixture();
        let full = registry.load_full("rust-debug").unwrap();
        assert!(full.body.contains("cargo check"), "level-2 body");
        assert_eq!(full.references.len(), 1, "level-3 references");
        assert_eq!(full.references[0].0, "linker.md");
        assert_eq!(full.scripts.len(), 1);
    }

    #[test]
    fn uninstalled_skills_error_on_full_load() {
        let registry = SkillRegistry::new();
        assert!(registry.load_full("ghost").is_err());
    }

    #[test]
    fn empty_directory_loads_zero() {
        let temp = tempfile::tempdir().unwrap();
        let registry = SkillRegistry::new();
        assert_eq!(registry.load_directory(temp.path()).unwrap(), 0);
        assert!(registry.summaries().is_empty());
    }
}
