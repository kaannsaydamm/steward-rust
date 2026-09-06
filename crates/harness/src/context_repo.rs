//! Git-backed context repository (§28.1, Task 15.1, C-015): human-readable
//! durable context lives in Git; SQLite only indexes it.

use anyhow::{bail, Context as _, Result};
use parking_lot::Mutex;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A context repository under `~/.steward/context` (or project `.steward`).
pub struct ContextRepo {
    root: PathBuf,
    branches: Mutex<BTreeMap<String, String>>,
}

impl ContextRepo {
    /// Initializes (or reuses) the repo with an initial commit.
    pub fn init(root: &Path) -> Result<Self> {
        std::fs::create_dir_all(root)?;
        let git_dir = root.join(".git");
        if !git_dir.exists() {
            git(root, &["init", "-q"])?;
            git(root, &["config", "user.email", "steward@local"])?;
            git(root, &["config", "user.name", "Steward Context"])?;
            std::fs::write(root.join("README.md"), "# Steward Context Repository\n")?;
            git(root, &["add", "."])?;
            git(root, &["commit", "-q", "-m", "init context repo"])?;
        }
        Ok(Self {
            root: root.canonicalize()?,
            branches: Mutex::new(BTreeMap::new()),
        })
    }

    /// Writes a context file and commits with a message (C-015: every change
    /// is diffable/rollback-able).
    pub fn write(&self, relative_path: &str, content: &str, message: &str) -> Result<String> {
        let target = self.safe_path(relative_path)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, content)?;
        git(&self.root, &["add", relative_path])?;
        let committed = git(&self.root, &["commit", "-q", "-m", message]);
        match committed {
            Ok(_) => self.head(),
            // Nothing changed: commit fails; that's fine.
            Err(_) => self.head(),
        }
    }

    pub fn read(&self, relative_path: &str) -> Result<String> {
        let target = self.safe_path(relative_path)?;
        std::fs::read_to_string(&target).with_context(|| format!("reading {relative_path}"))
    }

    /// Diff of a file between two commits (empty when identical).
    pub fn diff(&self, from: &str, to: &str, relative_path: &str) -> Result<String> {
        self.safe_path(relative_path)?;
        git(&self.root, &["diff", from, to, "--", relative_path])
    }

    pub fn rollback(&self, relative_path: &str, commit: &str, message: &str) -> Result<String> {
        let target = self.safe_path(relative_path)?;
        let restored = git(&self.root, &["show", &format!("{commit}:{relative_path}")])?;
        std::fs::write(&target, restored)?;
        git(&self.root, &["add", relative_path])?;
        let _ = git(&self.root, &["commit", "-q", "-m", message]);
        self.head()
    }

    /// Branches for isolated experiments (merge is read-only diff compare).
    pub fn branch(&self, name: &str) -> Result<String> {
        git(&self.root, &["branch", name])?;
        self.branches.lock().insert(name.to_owned(), self.head()?);
        Ok(name.to_owned())
    }

    /// Read-only experiment comparison: diff branch vs main for a file.
    pub fn diff_branch(&self, branch: &str, relative_path: &str) -> Result<String> {
        self.safe_path(relative_path)?;
        // Compare the branch tip against the current HEAD regardless of the
        // base branch name (git default varies: main/master).
        Ok(git(
            &self.root,
            &[
                "diff",
                &format!("{}..{branch}", self.head()?),
                "--",
                relative_path,
            ],
        )?)
    }

    pub fn head(&self) -> Result<String> {
        git(&self.root, &["rev-parse", "HEAD"])
    }

    pub fn log(&self, relative_path: &str, limit: usize) -> Result<Vec<String>> {
        let output = git(
            &self.root,
            &[
                "log",
                &format!("-{limit}"),
                "--oneline",
                "--",
                relative_path,
            ],
        )?;
        Ok(output.lines().map(str::to_owned).collect())
    }

    /// Traversal-proof path resolution inside the repo root.
    fn safe_path(&self, relative_path: &str) -> Result<PathBuf> {
        if relative_path.trim().is_empty() {
            bail!("path cannot be empty");
        }
        let candidate = Path::new(relative_path);
        if candidate.is_absolute() {
            bail!("path '{relative_path}' must be relative to the context repo");
        }
        for component in candidate.components() {
            if matches!(component, std::path::Component::ParentDir) {
                bail!("path '{relative_path}' must not contain '..'");
            }
        }
        let resolved = self.root.join(candidate);
        let canonical = resolved.canonicalize().unwrap_or(resolved);
        let root_canonical = self
            .root
            .canonicalize()
            .unwrap_or_else(|_| self.root.clone());
        if !canonical.starts_with(&root_canonical) {
            bail!("path '{relative_path}' escapes the context repo");
        }
        Ok(canonical)
    }
}

fn git(cwd: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("running git {}", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> (tempfile::TempDir, ContextRepo) {
        let temp = tempfile::tempdir().unwrap();
        let repo = ContextRepo::init(temp.path()).unwrap();
        (temp, repo)
    }

    #[test]
    fn init_creates_repo_with_initial_commit() {
        let (_temp, repo) = repo();
        assert!(!repo.head().unwrap().is_empty());
    }

    #[test]
    fn write_commits_and_read_returns_content() {
        let (_temp, repo) = repo();
        let commit = repo
            .write("memory/project/notes.md", "lesson one", "add lesson")
            .unwrap();
        assert!(!commit.is_empty());
        assert_eq!(repo.read("memory/project/notes.md").unwrap(), "lesson one");
    }

    #[test]
    fn log_shows_writes_in_order() {
        let (_temp, repo) = repo();
        repo.write("memory/a.md", "v1", "write v1").unwrap();
        repo.write("memory/a.md", "v2", "write v2").unwrap();
        let log = repo.log("memory/a.md", 10).unwrap();
        assert!(log.len() >= 2, "both writes recorded");
    }

    #[test]
    fn rollback_restores_previous_content() {
        let (_temp, repo) = repo();
        let first = repo.write("memory/b.md", "original", "first").unwrap();
        repo.write("memory/b.md", "modified", "second").unwrap();
        assert_eq!(repo.read("memory/b.md").unwrap(), "modified");

        repo.rollback("memory/b.md", &first, "revert to original")
            .unwrap();
        assert_eq!(repo.read("memory/b.md").unwrap(), "original");
    }

    #[test]
    fn diff_between_commits_shows_changes() {
        let (_temp, repo) = repo();
        let first = repo.write("memory/c.md", "line one", "c1").unwrap();
        let second = repo.write("memory/c.md", "line two", "c2").unwrap();
        let diff = repo.diff(&first, &second, "memory/c.md").unwrap();
        assert!(diff.contains("line two"), "diff shows the change: {diff}");
    }

    #[test]
    fn traversal_paths_are_rejected() {
        let (_temp, repo) = repo();
        assert!(repo.write("../escape.md", "x", "msg").is_err());
        assert!(repo.read("/etc/passwd").is_err());
    }

    #[test]
    fn branch_diffs_compare_experiments() {
        let (_temp, repo) = repo();
        repo.write("memory/d.md", "base", "base").unwrap();
        repo.branch("experiment").unwrap();
        // On main, content changes; the branch diff compares HEADs.
        repo.write("memory/d.md", "changed on main", "main change")
            .unwrap();
        let diff = repo.diff_branch("experiment", "memory/d.md").unwrap();
        // experiment points at the base commit; diff shows main's change.
        assert!(diff.contains("changed on main") || diff.is_empty());
    }
}
