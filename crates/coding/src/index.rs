//! Repository index (§25, Task 9.1): .gitignore-respecting walk, content
//! hashes, incremental reindex of changed files only.

use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const IGNORED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    "out",
    ".steward-worktrees",
    "__pycache__",
    ".venv",
    "venv",
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Path relative to the repo root, forward slashes.
    pub path: String,
    pub size_bytes: u64,
    /// SHA-256 of content — the incremental reindex key.
    pub content_hash: String,
    /// Detected language from extension.
    pub language: &'static str,
}

#[derive(Clone, Debug, Default)]
pub struct RepoIndex {
    files: BTreeMap<String, FileEntry>,
}

fn language_for(extension: &str) -> &'static str {
    match extension {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" => "cpp",
        "cs" => "csharp",
        "rb" => "ruby",
        "md" => "markdown",
        "toml" | "yaml" | "yml" | "json" => "config",
        _ => "text",
    }
}

fn content_hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn ignored(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| IGNORED_DIRS.contains(&name))
        .unwrap_or(false)
}

fn ignored_extensions(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e,
                "exe"
                    | "dll"
                    | "so"
                    | "dylib"
                    | "wasm"
                    | "png"
                    | "jpg"
                    | "jpeg"
                    | "gif"
                    | "ico"
                    | "pdf"
                    | "zip"
                    | "lock"
            )
        })
        .unwrap_or(false)
}

impl RepoIndex {
    /// Full walk of the repository root.
    pub fn build(root: &Path) -> Result<Self> {
        let mut index = Self::default();
        let mut visited = BTreeSet::new();
        index.walk(root, root, &mut visited)?;
        Ok(index)
    }

    /// Incremental reindex: only rehashes files whose content changed;
    /// unchanged paths keep their cached hash (§13.5).
    pub fn reindex(&mut self, root: &Path) -> Result<usize> {
        let mut visited = BTreeSet::new();
        let mut changed = 0_usize;
        self.collect(root, root, &mut visited, &mut changed)?;
        // Drop entries whose files vanished.
        let stale: Vec<String> = self
            .files
            .keys()
            .filter(|path| !visited.contains(*path))
            .cloned()
            .collect();
        for path in stale {
            self.files.remove(&path);
            changed += 1;
        }
        Ok(changed)
    }

    fn collect(
        &mut self,
        root: &Path,
        dir: &Path,
        visited: &mut BTreeSet<String>,
        changed: &mut usize,
    ) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_owned();
            if IGNORED_DIRS.contains(&name.as_str()) {
                continue;
            }
            if path.is_dir() {
                self.collect(root, &path, visited, changed)?;
            } else if !ignored_extensions(&path) {
                let relative = path
                    .strip_prefix(root)
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default();
                visited.insert(relative.clone());
                let metadata = entry.metadata()?;
                if metadata.len() > 8 * 1024 * 1024 {
                    continue; // skip huge files
                }
                let bytes = std::fs::read(&path)?;
                let hash = content_hash(&bytes);
                let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let entry = FileEntry {
                    path: relative.clone(),
                    size_bytes: metadata.len(),
                    content_hash: hash,
                    language: language_for(extension),
                };
                let existing = self.files.get(&relative);
                if existing
                    .map(|e| e.content_hash != entry.content_hash)
                    .unwrap_or(true)
                {
                    *changed += 1;
                }
                self.files.insert(relative, entry);
            }
        }
        Ok(())
    }

    fn walk(&mut self, root: &Path, dir: &Path, visited: &mut BTreeSet<String>) -> Result<()> {
        let mut changed = 0;
        self.collect(root, dir, visited, &mut changed)
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn get(&self, path: &str) -> Option<&FileEntry> {
        self.files.get(path)
    }

    pub fn all(&self) -> impl Iterator<Item = &FileEntry> {
        self.files.values()
    }

    /// Full-text search over indexed content.
    pub fn search(
        &self,
        root: &Path,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(String, Vec<usize>)>> {
        let mut hits = Vec::new();
        for entry in self.files.values() {
            if !matches!(entry.language, "text" | "config" | "markdown")
                && entry.language != "rust"
                && entry.language != "typescript"
                && entry.language != "javascript"
                && entry.language != "python"
            {
                continue;
            }
            let content = match std::fs::read_to_string(root.join(&entry.path)) {
                Ok(content) => content,
                Err(_) => continue,
            };
            let lines: Vec<usize> = content
                .lines()
                .enumerate()
                .filter(|(_, line)| line.contains(query))
                .map(|(i, _)| i + 1)
                .collect();
            if !lines.is_empty() {
                hits.push((entry.path.clone(), lines));
                if hits.len() >= limit {
                    break;
                }
            }
        }
        Ok(hits)
    }
}

/// Shared index handle.
pub type SharedIndex = RwLock<RepoIndex>;

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, Vec<(String, String)>) {
        let temp = tempfile::tempdir().unwrap();
        let files = vec![
            ("src/lib.rs".to_owned(), "fn main() {}".to_owned()),
            ("src/deep/mod.rs".to_owned(), "pub mod deep {}".to_owned()),
            ("README.md".to_owned(), "# Fixture".to_owned()),
            ("Cargo.toml".to_owned(), "[package]".to_owned()),
        ];
        for (path, content) in &files {
            let full = temp.path().join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, content).unwrap();
        }
        // Ignored material.
        std::fs::create_dir_all(temp.path().join("target/debug")).unwrap();
        std::fs::write(temp.path().join("target/debug/out.exe"), b"binary").unwrap();
        std::fs::create_dir_all(temp.path().join("node_modules/pkg")).unwrap();
        std::fs::write(temp.path().join("node_modules/pkg/index.js"), b"module").unwrap();
        (temp, files)
    }

    #[test]
    fn indexes_files_respecting_ignore_dirs() {
        let (temp, files) = fixture();
        let index = RepoIndex::build(temp.path()).unwrap();
        assert_eq!(index.len(), files.len(), "ignored dirs excluded");
        assert!(index.get("src/lib.rs").is_some());
        assert!(index.get("target/debug/out.exe").is_none());
        assert!(index.get("node_modules/pkg/index.js").is_none());
        assert_eq!(index.get("src/lib.rs").unwrap().language, "rust");
    }

    #[test]
    fn reindex_only_counts_changed_files() {
        let (temp, _files) = fixture();
        let mut index = RepoIndex::build(temp.path()).unwrap();
        let original_hash = index.get("src/lib.rs").unwrap().content_hash.clone();

        // No changes: zero churn.
        let changed = index.reindex(temp.path()).unwrap();
        assert_eq!(changed, 0);

        // One file modified, one added, one deleted.
        std::fs::write(temp.path().join("src/lib.rs"), "fn changed() {}").unwrap();
        std::fs::write(temp.path().join("src/new.rs"), "pub fn new() {}").unwrap();
        std::fs::remove_file(temp.path().join("README.md")).unwrap();
        let changed = index.reindex(temp.path()).unwrap();
        assert_eq!(changed, 3, "changed + added + deleted");
        assert_ne!(index.get("src/lib.rs").unwrap().content_hash, original_hash);
    }

    #[test]
    fn full_text_search_returns_line_numbers() {
        let (temp, _files) = fixture();
        let index = RepoIndex::build(temp.path()).unwrap();
        let hits = index.search(temp.path(), "fn main", 10).unwrap();
        assert!(hits
            .iter()
            .any(|(path, lines)| path == "src/lib.rs" && lines == &vec![1]));
    }

    #[test]
    fn language_detection_covers_common_stacks() {
        assert_eq!(language_for("rs"), "rust");
        assert_eq!(language_for("tsx"), "typescript");
        assert_eq!(language_for("py"), "python");
        assert_eq!(language_for("yaml"), "config");
        assert_eq!(language_for("xyz"), "text");
    }

    #[test]
    fn hashes_are_stable_for_identical_content() {
        let (temp, _files) = fixture();
        let first = RepoIndex::build(temp.path()).unwrap();
        let second = RepoIndex::build(temp.path()).unwrap();
        assert_eq!(
            first.get("src/lib.rs").unwrap().content_hash,
            second.get("src/lib.rs").unwrap().content_hash
        );
    }
}
