//! Git worktree manager (§24.2, Task 8.3, D-015): parallel writers get
//! distinct worktrees, deterministic cleanup, and per-worktree diffs.

use anyhow::{bail, Context as _, Result};
use parking_lot::Mutex;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug)]
pub struct WorktreeHandle {
    pub worktree_id: String,
    pub branch: String,
    pub path: PathBuf,
    pub base_commit: String,
}

pub struct WorktreeManager {
    worktrees: Mutex<BTreeMap<String, WorktreeHandle>>,
}

impl Default for WorktreeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WorktreeManager {
    pub fn new() -> Self {
        Self {
            worktrees: Mutex::new(BTreeMap::new()),
        }
    }

    /// Creates a unique branch + worktree under `<repo>/.steward-worktrees/`.
    pub fn create(&self, repo_root: &Path, worktree_id: &str) -> Result<WorktreeHandle> {
        {
            let worktrees = self.worktrees.lock();
            if worktrees.contains_key(worktree_id) {
                bail!("worktree '{worktree_id}' already exists");
            }
        }
        let base_commit = git(repo_root, &["rev-parse", "HEAD"])?;
        let branch = format!("steward/{worktree_id}");
        let path = repo_root.join(".steward-worktrees").join(worktree_id);

        // Branch from HEAD (ok if it already exists from a prior crashed run).
        let _ = git(repo_root, &["branch", &branch, &base_commit]);
        let _ = git(
            repo_root,
            &["worktree", "add", &path.to_string_lossy(), &branch],
        )?;

        let handle = WorktreeHandle {
            worktree_id: worktree_id.to_owned(),
            branch,
            path: path.canonicalize().unwrap_or(path),
            base_commit,
        };
        self.worktrees
            .lock()
            .insert(worktree_id.to_owned(), handle.clone());
        Ok(handle)
    }

    pub fn get(&self, worktree_id: &str) -> Option<WorktreeHandle> {
        self.worktrees.lock().get(worktree_id).cloned()
    }

    pub fn active_count(&self) -> usize {
        self.worktrees.lock().len()
    }

    /// Removes the worktree (and its branch) — never touches paths outside
    /// the registered worktree directory (D-015 cleanup guarantee).
    pub fn remove(&self, repo_root: &Path, worktree_id: &str) -> Result<bool> {
        let handle = {
            let mut worktrees = self.worktrees.lock();
            match worktrees.remove(worktree_id) {
                Some(handle) => handle,
                None => return Ok(false),
            }
        };
        let canonical_worktree = handle
            .path
            .canonicalize()
            .unwrap_or_else(|_| handle.path.clone());
        let allowed_root = repo_root
            .join(".steward-worktrees")
            .canonicalize()
            .unwrap_or_else(|_| repo_root.join(".steward-worktrees"));
        if !canonical_worktree.starts_with(&allowed_root) {
            bail!(
                "refusing to remove worktree outside managed directory: {}",
                canonical_worktree.display()
            );
        }
        let _ = git(
            repo_root,
            &[
                "worktree",
                "remove",
                "--force",
                &canonical_worktree.to_string_lossy(),
            ],
        );
        let _ = git(repo_root, &["branch", "-D", &handle.branch]);
        Ok(true)
    }

    /// Diff of a worktree against its base commit (per-worker evidence).
    pub fn diff(&self, worktree_id: &str) -> Result<String> {
        let handle = self.get(worktree_id).context("worktree not found")?;
        git(&handle.path, &["diff", &handle.base_commit])
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

    fn init_repo() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        git(root, &["init", "-q"]).unwrap();
        git(root, &["config", "user.email", "test@steward.local"]).unwrap();
        git(root, &["config", "user.name", "Steward Test"]).unwrap();
        std::fs::write(root.join("README.md"), "seed").unwrap();
        git(root, &["add", "."]).unwrap();
        git(root, &["commit", "-q", "-m", "seed"]).unwrap();
        temp
    }

    #[test]
    fn two_writers_receive_distinct_worktrees() {
        let repo = init_repo();
        let manager = WorktreeManager::new();

        let a = manager.create(repo.path(), "worker-a").unwrap();
        let b = manager.create(repo.path(), "worker-b").unwrap();

        assert_ne!(a.path, b.path);
        assert_ne!(a.branch, b.branch);
        assert_eq!(manager.active_count(), 2);
        assert!(a.path.join(".git").exists());
        assert!(b.path.join(".git").exists());
    }

    #[test]
    fn duplicate_worktree_ids_are_rejected() {
        let repo = init_repo();
        let manager = WorktreeManager::new();
        manager.create(repo.path(), "w1").unwrap();
        assert!(manager.create(repo.path(), "w1").is_err());
    }

    #[test]
    fn per_worktree_diff_is_isolated() {
        let repo = init_repo();
        let manager = WorktreeManager::new();
        let a = manager.create(repo.path(), "worker-a").unwrap();
        let b = manager.create(repo.path(), "worker-b").unwrap();

        // Worker A edits its own copy; B must stay clean.
        std::fs::write(a.path.join("README.md"), "changed by a").unwrap();

        let diff_a = manager.diff("worker-a").unwrap();
        assert!(diff_a.contains("changed by a"), "A's diff shows its edit");
        let diff_b = manager.diff("worker-b").unwrap().trim().to_owned();
        assert!(diff_b.is_empty(), "B must have no diff, got: {diff_b}");
        let _ = b;
    }

    #[test]
    fn cleanup_removes_worktree_and_branch() {
        let repo = init_repo();
        let manager = WorktreeManager::new();
        let handle = manager.create(repo.path(), "worker-a").unwrap();
        assert!(handle.path.exists());

        assert!(manager.remove(repo.path(), "worker-a").unwrap());
        assert!(!handle.path.exists());
        assert_eq!(manager.active_count(), 0);
        // Branch is gone.
        assert!(git(repo.path(), &["rev-parse", "--verify", &handle.branch]).is_err());
    }

    #[test]
    fn cleanup_refuses_paths_outside_managed_root() {
        let repo = init_repo();
        let manager = WorktreeManager::new();
        // Manually inject a handle pointing outside the managed directory.
        manager.worktrees.lock().insert(
            "evil".into(),
            WorktreeHandle {
                worktree_id: "evil".into(),
                branch: "main".into(),
                path: std::path::PathBuf::from("C:/Windows"),
                base_commit: "HEAD".into(),
            },
        );
        let result = manager.remove(repo.path(), "evil");
        assert!(result.is_err(), "outside paths must be refused");
    }

    #[test]
    fn removing_unknown_worktree_returns_false() {
        let repo = init_repo();
        let manager = WorktreeManager::new();
        assert!(!manager.remove(repo.path(), "missing").unwrap());
    }
}
