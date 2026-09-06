//! Symbol graph + repo map (§25.6, Task 9.4, C-008/D-011).
//!
//! Language-aware *heuristic* symbol extraction (fn/class/struct/def
//! detection) + import edges, ranked by degree centrality, packed within a
//! token budget. Tree-sitter replaces the scanners behind the same types
//! (Phase 10.4) without changing the map contract.

use crate::index::RepoIndex;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SymbolEntry {
    pub name: String,
    pub kind: SymbolKind,
    pub file: String,
    pub line: u32,
    /// Files this symbol's file imports (edges).
    pub imports: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Struct,
    Class,
    Trait,
    Impl,
    Enum,
}

#[derive(Clone, Debug, Default)]
pub struct RepoMap {
    symbols: Vec<SymbolEntry>,
}

impl RepoMap {
    /// Scans indexed files and extracts symbols with import edges.
    pub fn build(index: &RepoIndex, root: &Path) -> Result<Self> {
        let mut symbols = Vec::new();
        for entry in index.all() {
            let content = match std::fs::read_to_string(root.join(&entry.path)) {
                Ok(content) => content,
                Err(_) => continue,
            };
            symbols.extend(extract(&entry.path, entry.language, &content));
        }
        Ok(Self { symbols })
    }

    pub fn symbols(&self) -> &[SymbolEntry] {
        &self.symbols
    }

    /// Degree centrality: symbols in files with the most import edges first.
    fn ranked(&self) -> Vec<&SymbolEntry> {
        let mut degree: BTreeMap<String, usize> = BTreeMap::new();
        for symbol in &self.symbols {
            *degree.entry(symbol.file.clone()).or_default() += symbol.imports.len();
        }
        let mut ranked: Vec<&SymbolEntry> = self.symbols.iter().collect();
        ranked.sort_by(|a, b| {
            let da = degree.get(&a.file).copied().unwrap_or(0);
            let db = degree.get(&b.file).copied().unwrap_or(0);
            db.cmp(&da).then_with(|| a.name.cmp(&b.name))
        });
        ranked
    }

    /// Ranked map within a token budget (C-008): one line per symbol,
    /// `kind name (file:line)`.
    pub fn render(&self, token_budget: u32) -> String {
        let mut lines: Vec<String> = Vec::new();
        let mut used = 0_u32;
        for symbol in self.ranked() {
            let line = format!("{:?} {} ({}:{})", symbol.kind, symbol.name, symbol.file, symbol.line)
                .to_lowercase();
            let tokens = (line.len() as u32).div_ceil(4);
            if used + tokens > token_budget {
                break;
            }
            used += tokens;
            lines.push(line);
        }
        lines.join("\n")
    }

    /// Lookup by name across the graph (D-011 workspace symbols).
    pub fn find(&self, name: &str) -> Vec<&SymbolEntry> {
        let needle = name.to_lowercase();
        self.symbols
            .iter()
            .filter(|s| s.name.to_lowercase().contains(&needle))
            .collect()
    }
}

/// Heuristic extraction per language family.
fn extract(path: &str, language: &str, content: &str) -> Vec<SymbolEntry> {
    let imports: Vec<String> = content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("use ") {
                Some(rest.trim_end_matches(';').split("::").next().unwrap_or(rest).to_owned())
            } else if let Some(rest) = trimmed.strip_prefix("import ") {
                Some(rest.trim_start_matches(|c: char| !c.is_alphanumeric()).to_owned())
            } else {
                None
            }
        })
        .collect();

    let mut symbols = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        let mut kind = None;
        let mut name = None;
        if language == "rust" {
            if let Some(rest) = trimmed.strip_prefix("fn ") {
                kind = Some(SymbolKind::Function);
                name = rest.split(|c: char| c == '(' || c == ' ' || c == '<').next();
            } else if let Some(rest) = trimmed.strip_prefix("pub fn ") {
                kind = Some(SymbolKind::Function);
                name = rest.split(|c: char| c == '(' || c == ' ' || c == '<').next();
            } else if let Some(rest) = trimmed.strip_prefix("struct ") {
                kind = Some(SymbolKind::Struct);
                name = rest.split(|c: char| !(c.is_alphanumeric() || c == '_')).next();
            } else if let Some(rest) = trimmed.strip_prefix("enum ") {
                kind = Some(SymbolKind::Enum);
                name = rest.split(|c: char| !(c.is_alphanumeric() || c == '_')).next();
            } else if let Some(rest) = trimmed.strip_prefix("trait ") {
                kind = Some(SymbolKind::Trait);
                name = rest.split(|c: char| !(c.is_alphanumeric() || c == '_')).next();
            } else if let Some(rest) = trimmed.strip_prefix("impl ") {
                kind = Some(SymbolKind::Impl);
                name = rest.split(" for ").last().and_then(|n| n.split(|c: char| !(c.is_alphanumeric() || c == '_')).next());
            }
        } else if matches!(language, "typescript" | "javascript") {
            if let Some(rest) = trimmed.strip_prefix("function ") {
                kind = Some(SymbolKind::Function);
                name = rest.split('(').next();
            } else if let Some(rest) = trimmed.strip_prefix("class ") {
                kind = Some(SymbolKind::Class);
                name = rest.split(|c: char| !(c.is_alphanumeric() || c == '_')).next();
            }
        } else if language == "python" {
            if let Some(rest) = trimmed.strip_prefix("def ") {
                kind = Some(SymbolKind::Function);
                name = rest.split('(').next();
            } else if let Some(rest) = trimmed.strip_prefix("class ") {
                kind = Some(SymbolKind::Class);
                name = rest.split(|c: char| c == ':' || c == '(').next();
            }
        }
        if let (Some(kind), Some(name)) = (kind, name) {
            let name = name.trim().to_owned();
            if !name.is_empty() {
                symbols.push(SymbolEntry {
                    name,
                    kind,
                    file: path.to_owned(),
                    line: (index + 1) as u32,
                    imports: imports.clone(),
                });
            }
        }
    }
    symbols
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_repo() -> (tempfile::TempDir, RepoIndex) {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("src/core")).unwrap();
        std::fs::write(
            temp.path().join("src/lib.rs"),
            "use serde_json;\nuse anyhow;\npub fn run_kernel() {}\nstruct EngineState {}\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("src/core/mod.rs"),
            "use crate;\npub fn spawn_agent() {}\ntrait Scheduler {}\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("src/app.ts"),
            "import { Router } from 'express';\nfunction startServer() {}\nclass App {}\n",
        )
        .unwrap();
        let index = RepoIndex::build(temp.path()).unwrap();
        (temp, index)
    }

    #[test]
    fn extracts_rust_and_ts_symbols() {
        let (temp, index) = fixture_repo();
        let map = RepoMap::build(&index, temp.path()).unwrap();
        let names: Vec<&str> = map.symbols().iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"run_kernel"));
        assert!(names.contains(&"EngineState"));
        assert!(names.contains(&"spawn_agent"));
        assert!(names.contains(&"Scheduler"));
        assert!(names.contains(&"startServer"));
        assert!(names.contains(&"App"));
    }

    #[test]
    fn import_edges_are_captured() {
        let (temp, index) = fixture_repo();
        let map = RepoMap::build(&index, temp.path()).unwrap();
        let kernel = map.symbols().iter().find(|s| s.name == "run_kernel").unwrap();
        assert!(kernel.imports.contains(&"serde_json".to_owned()));
        assert!(kernel.imports.contains(&"anyhow".to_owned()));
    }

    #[test]
    fn repo_map_respects_token_budget() {
        let (temp, index) = fixture_repo();
        let map = RepoMap::build(&index, temp.path()).unwrap();
        let small = map.render(10);
        let large = map.render(10_000);
        assert!(small.lines().count() < large.lines().count(), "budget limits output");
        assert!((small.len() as u32).div_ceil(4) <= 10 + 4, "small budget keeps output tiny");
    }

    #[test]
    fn find_locates_symbols_case_insensitively() {
        let (temp, index) = fixture_repo();
        let map = RepoMap::build(&index, temp.path()).unwrap();
        assert_eq!(map.find("KERNEL").len(), 1);
        assert_eq!(map.find("scheduler").len(), 1);
        assert!(map.find("zzz").is_empty());
    }

    #[test]
    fn empty_index_yields_empty_map() {
        let map = RepoMap::default();
        assert!(map.symbols().is_empty());
        assert_eq!(map.render(1000), "");
    }
}
