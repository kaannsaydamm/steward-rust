//! Settings for external chat-channel bridges (currently Telegram). The token lives in
//! `~/.steward/channels.json` — the same local trust boundary as the rest of Steward's state.
//! Chat access is default-deny: an empty allowlist means the bridge answers unknown chats only
//! with pairing instructions, never with model output.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChannelSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telegram_token: Option<String>,
    #[serde(default)]
    pub telegram_allowed_chats: Vec<i64>,
}

impl ChannelSettings {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = fs::read(path)
            .with_context(|| format!("reading channel settings from {}", path.display()))?;
        serde_json::from_slice(&bytes)
            .with_context(|| format!("parsing channel settings from {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .context("channel settings path has no parent")?;
        fs::create_dir_all(parent)
            .with_context(|| format!("creating channel directory {}", parent.display()))?;
        let temporary = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self)?;
        fs::write(&temporary, bytes)
            .with_context(|| format!("writing channel settings to {}", temporary.display()))?;
        fs::rename(&temporary, path)
            .with_context(|| format!("replacing channel settings at {}", path.display()))
    }

    pub fn is_chat_allowed(&self, chat_id: i64) -> bool {
        self.telegram_allowed_chats.contains(&chat_id)
    }

    pub fn allow_chat(&mut self, chat_id: i64) {
        if !self.telegram_allowed_chats.contains(&chat_id) {
            self.telegram_allowed_chats.push(chat_id);
            self.telegram_allowed_chats.sort_unstable();
        }
    }

    pub fn disallow_chat(&mut self, chat_id: i64) {
        self.telegram_allowed_chats.retain(|id| *id != chat_id);
    }
}

pub fn settings_path() -> Result<PathBuf> {
    let root = crate::storage::root().context("resolving ~/.steward data root")?;
    Ok(root.join("channels.json"))
}

#[cfg(test)]
mod tests {
    use super::ChannelSettings;

    #[test]
    fn chats_are_denied_by_default() {
        let settings = ChannelSettings::default();
        assert!(!settings.is_chat_allowed(42));
    }

    #[test]
    fn allowed_chats_round_trip_through_disk() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("channels.json");
        let mut settings = ChannelSettings {
            telegram_token: Some("123:abc".to_owned()),
            ..Default::default()
        };
        settings.allow_chat(42);
        settings.save(&path).expect("save settings");

        let loaded = ChannelSettings::load(&path).expect("load settings");

        assert!(loaded.is_chat_allowed(42));
        assert_eq!(loaded.telegram_token.as_deref(), Some("123:abc"));
    }

    #[test]
    fn disallow_removes_a_chat() {
        let mut settings = ChannelSettings::default();
        settings.allow_chat(42);
        settings.disallow_chat(42);
        assert!(!settings.is_chat_allowed(42));
    }
}
