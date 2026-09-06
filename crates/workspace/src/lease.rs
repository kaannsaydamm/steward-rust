//! Workspace leases (§24.1, Task 8.1): canonical roots, modes, and
//! traversal-proof path resolution.

use anyhow::{bail, Context as _, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceMode {
    ReadOnly,
    ReadWrite,
    Worktree,
    EphemeralCopy,
    Container,
    SshRemote,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceLease {
    pub workspace_id: String,
    /// Canonicalized root — all path checks anchor here.
    pub root: PathBuf,
    pub mode: WorkspaceMode,
    /// Owning run, when leased by a run.
    pub run_id: Option<String>,
}

impl WorkspaceLease {
    /// Resolves a relative path inside the lease; refuses absolute paths,
    /// `..` traversal, and symlink escapes (Task 8.1 acceptance).
    pub fn resolve(&self, relative: &str) -> Result<PathBuf> {
        if relative.trim().is_empty() {
            bail!("path cannot be empty");
        }
        let candidate = Path::new(relative);
        if candidate.is_absolute() {
            bail!("path '{relative}' must be relative to the workspace");
        }
        for component in candidate.components() {
            if matches!(component, std::path::Component::ParentDir) {
                bail!("path '{relative}' must not contain '..'");
            }
        }
        let resolved = self.root.join(candidate);
        let canonical = resolved
            .canonicalize()
            .with_context(|| format!("resolving '{relative}'"))?;
        let root_canonical = self
            .root
            .canonicalize()
            .unwrap_or_else(|_| self.root.clone());
        if !canonical.starts_with(&root_canonical) {
            bail!("path '{relative}' escapes the workspace root");
        }
        Ok(canonical)
    }

    pub fn writable(&self) -> bool {
        matches!(
            self.mode,
            WorkspaceMode::ReadWrite | WorkspaceMode::Worktree | WorkspaceMode::EphemeralCopy
        )
    }
}

/// Registry of active workspaces with canonicalization at registration.
pub struct WorkspaceRegistry {
    workspaces: Mutex<BTreeMap<String, WorkspaceLease>>,
}

impl Default for WorkspaceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceRegistry {
    pub fn new() -> Self {
        Self { workspaces: Mutex::new(BTreeMap::new()) }
    }

    /// Registers a workspace after canonicalizing its root.
    pub fn register(
        &self,
        workspace_id: &str,
        root: &Path,
        mode: WorkspaceMode,
        run_id: Option<&str>,
    ) -> Result<WorkspaceLease> {
        let canonical = root
            .canonicalize()
            .with_context(|| format!("canonicalizing workspace root {}", root.display()))?;
        let lease = WorkspaceLease {
            workspace_id: workspace_id.to_owned(),
            root: canonical,
            mode,
            run_id: run_id.map(str::to_owned),
        };
        self.workspaces.lock().insert(workspace_id.to_owned(), lease.clone());
        Ok(lease)
    }

    pub fn get(&self, workspace_id: &str) -> Option<WorkspaceLease> {
        self.workspaces.lock().get(workspace_id).cloned()
    }

    pub fn release(&self, workspace_id: &str) -> bool {
        self.workspaces.lock().remove(workspace_id).is_some()
    }

    pub fn active_count(&self) -> usize {
        self.workspaces.lock().len()
    }
}

/// Convenience: shared registry handle.
pub type SharedRegistry = Arc<WorkspaceRegistry>;

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, SharedRegistry) {
        let temp = tempfile::tempdir().unwrap();
        let registry = Arc::new(WorkspaceRegistry::new());
        registry.register("ws1", temp.path(), WorkspaceMode::ReadWrite, None).unwrap();
        (temp, registry)
    }

    #[test]
    fn registration_canonicalizes_root() {
        let (temp, registry) = setup();
        let lease = registry.get("ws1").unwrap();
        let expected = temp.path().canonicalize().unwrap();
        assert_eq!(lease.root, expected);
        assert!(lease.writable());
    }

    #[test]
    fn absolute_paths_are_rejected() {
        let (_temp, registry) = setup();
        let lease = registry.get("ws1").unwrap();
        assert!(lease.resolve("/etc/passwd").is_err());
        assert!(lease.resolve("C:/Windows/system32").is_err());
    }

    #[test]
    fn dotdot_traversal_is_rejected() {
        let (_temp, registry) = setup();
        let lease = registry.get("ws1").unwrap();
        assert!(lease.resolve("../outside.txt").is_err());
        assert!(lease.resolve("sub/../../outside.txt").is_err());
    }

    #[test]
    fn symlink_escape_is_rejected() {
        let (temp, registry) = setup();
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("secret.txt");
        std::fs::write(&secret, "data").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&secret, temp.path().join("link.txt")).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&secret, temp.path().join("link.txt")).unwrap();

        let lease = registry.get("ws1").unwrap();
        // Symlinked targets resolve outside the root, so resolve() must reject.
        assert!(lease.resolve("link.txt").is_err(), "symlink escape must be rejected");
    }

    #[test]
    fn legitimate_relative_paths_resolve() {
        let (temp, registry) = setup();
        std::fs::create_dir_all(temp.path().join("src")).unwrap();
        std::fs::write(temp.path().join("src/lib.rs"), "fn main() {}").unwrap();

        let lease = registry.get("ws1").unwrap();
        let resolved = lease.resolve("src/lib.rs").unwrap();
        assert!(resolved.is_file());
        assert!(resolved.starts_with(temp.path().canonicalize().unwrap()));
    }

    #[test]
    fn empty_paths_are_rejected() {
        let (_temp, registry) = setup();
        let lease = registry.get("ws1").unwrap();
        assert!(lease.resolve("").is_err());
        assert!(lease.resolve("   ").is_err());
    }

    #[test]
    fn release_removes_the_lease() {
        let (_temp, registry) = setup();
        assert_eq!(registry.active_count(), 1);
        assert!(registry.release("ws1"));
        assert!(registry.get("ws1").is_none());
        assert_eq!(registry.active_count(), 0);
    }

    #[test]
    fn read_only_mode_blocks_writes() {
        let temp = tempfile::tempdir().unwrap();
        let registry = WorkspaceRegistry::new();
        registry.register("ro", temp.path(), WorkspaceMode::ReadOnly, None).unwrap();
        let lease = registry.get("ro").unwrap();
        assert!(!lease.writable());
    }
}
