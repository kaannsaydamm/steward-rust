use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SecuritySettings {
    pub process_exec_allowlist: Vec<String>,
}

impl SecuritySettings {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = fs::read(path)
            .with_context(|| format!("reading security settings from {}", path.display()))?;
        serde_json::from_slice(&bytes)
            .with_context(|| format!("parsing security settings from {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .context("security settings path has no parent")?;
        fs::create_dir_all(parent)
            .with_context(|| format!("creating security directory {}", parent.display()))?;
        let temporary = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self)?;
        fs::write(&temporary, bytes)
            .with_context(|| format!("writing security settings to {}", temporary.display()))?;
        fs::rename(&temporary, path)
            .with_context(|| format!("replacing security settings at {}", path.display()))
    }

    pub fn is_command_allowed(&self, command_line: &str) -> bool {
        if self.process_exec_allowlist.is_empty() {
            return true;
        }
        let program = command_line.split_whitespace().next().unwrap_or("");
        self.process_exec_allowlist
            .iter()
            .any(|allowed| allowed == program)
    }

    pub fn allow(&mut self, program: String) {
        if !self.process_exec_allowlist.contains(&program) {
            self.process_exec_allowlist.push(program);
            self.process_exec_allowlist.sort();
        }
    }

    pub fn disallow(&mut self, program: &str) {
        self.process_exec_allowlist.retain(|item| item != program);
    }
}

pub fn settings_path() -> Result<PathBuf> {
    let root = crate::storage::root().context("resolving ~/.steward data root")?;
    Ok(root.join("security.json"))
}

#[cfg(test)]
mod tests {
    use super::SecuritySettings;

    #[test]
    fn empty_allowlist_permits_any_command() {
        let settings = SecuritySettings::default();
        assert!(settings.is_command_allowed("rm -rf /"));
    }

    #[test]
    fn non_empty_allowlist_only_permits_listed_programs() {
        let mut settings = SecuritySettings::default();
        settings.allow("git".to_owned());

        assert!(settings.is_command_allowed("git status"));
        assert!(!settings.is_command_allowed("rm -rf /"));
    }

    #[test]
    fn disallow_removes_a_program() {
        let mut settings = SecuritySettings::default();
        settings.allow("git".to_owned());
        settings.disallow("git");

        assert!(settings.is_command_allowed("anything"));
    }
}
