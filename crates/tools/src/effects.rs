//! Effect taxonomy (§23.2): every side effect class the policy understands.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    FilesystemRead,
    FilesystemWrite,
    FilesystemDelete,
    ProcessExecute,
    ProcessSignal,
    GitRead,
    GitWrite,
    NetworkHttp,
    NetworkBrowser,
    CredentialRead,
    CredentialUse,
    ExternalMessage,
    PackageInstall,
    RemoteExecute,
    WorkspaceCreate,
    WorkspaceMerge,
    MemoryWrite,
    HarnessModify,
}

impl Effect {
    /// Human-readable name matching the plan's effect vocabulary.
    pub fn as_str(&self) -> &'static str {
        match self {
            Effect::FilesystemRead => "filesystem.read",
            Effect::FilesystemWrite => "filesystem.write",
            Effect::FilesystemDelete => "filesystem.delete",
            Effect::ProcessExecute => "process.execute",
            Effect::ProcessSignal => "process.signal",
            Effect::GitRead => "git.read",
            Effect::GitWrite => "git.write",
            Effect::NetworkHttp => "network.http",
            Effect::NetworkBrowser => "network.browser",
            Effect::CredentialRead => "credential.read",
            Effect::CredentialUse => "credential.use",
            Effect::ExternalMessage => "external.message",
            Effect::PackageInstall => "package.install",
            Effect::RemoteExecute => "remote.execute",
            Effect::WorkspaceCreate => "workspace.create",
            Effect::WorkspaceMerge => "workspace.merge",
            Effect::MemoryWrite => "memory.write",
            Effect::HarnessModify => "harness.modify",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_names_match_plan_vocabulary() {
        assert_eq!(Effect::FilesystemWrite.as_str(), "filesystem.write");
        assert_eq!(Effect::NetworkBrowser.as_str(), "network.browser");
        assert_eq!(Effect::HarnessModify.as_str(), "harness.modify");
    }

    #[test]
    fn effects_hash_for_policy_sets() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Effect::FilesystemRead);
        set.insert(Effect::FilesystemRead);
        assert_eq!(set.len(), 1);
    }
}
