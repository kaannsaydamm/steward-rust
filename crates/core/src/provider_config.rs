use anyhow::{bail, Context as _, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const SETTINGS_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProtocol {
    OpenAiChat,
    AnthropicMessages,
    GeminiGenerateContent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProviderProfile {
    pub profile_id: String,
    pub provider_id: String,
    pub display_name: String,
    pub protocol: ProviderProtocol,
    pub base_url: String,
    pub model: String,
    pub api_key_env: Option<String>,
    /// A key value saved directly into the profile so it survives daemon restarts without
    /// relying on the process environment. Takes precedence over `api_key_env` when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

impl ProviderProfile {
    pub fn validate(&self) -> Result<()> {
        validate_identifier("profile id", &self.profile_id)?;
        validate_identifier("provider id", &self.provider_id)?;
        if self.display_name.trim().is_empty() {
            bail!("provider display name cannot be empty");
        }
        let url = self.base_url.trim();
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            bail!("provider base URL must use http or https");
        }
        if self.model.trim().is_empty() {
            bail!("provider model cannot be empty");
        }
        if let Some(variable) = &self.api_key_env {
            validate_env_name(variable)?;
        }
        Ok(())
    }

    pub fn api_key(&self) -> Result<Option<String>> {
        if let Some(key) = &self.api_key {
            return Ok(Some(key.clone()));
        }
        match &self.api_key_env {
            Some(variable) => std::env::var(variable)
                .with_context(|| format!("environment variable {variable} is not set"))
                .map(Some),
            None => Ok(None),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProviderSettings {
    version: u32,
    pub active_profile: Option<String>,
    pub profiles: Vec<ProviderProfile>,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            active_profile: None,
            profiles: Vec::new(),
        }
    }
}

impl ProviderSettings {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = fs::read(path)
            .with_context(|| format!("reading provider profiles from {}", path.display()))?;
        let settings: Self = serde_json::from_slice(&bytes)
            .with_context(|| format!("parsing provider profiles from {}", path.display()))?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let parent = path
            .parent()
            .context("provider settings path has no parent")?;
        fs::create_dir_all(parent)
            .with_context(|| format!("creating provider directory {}", parent.display()))?;
        let temporary = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self)?;
        fs::write(&temporary, bytes)
            .with_context(|| format!("writing provider profiles to {}", temporary.display()))?;
        fs::rename(&temporary, path)
            .with_context(|| format!("replacing provider profiles at {}", path.display()))
    }

    pub fn upsert(&mut self, profile: ProviderProfile) -> Result<()> {
        profile.validate()?;
        if let Some(existing) = self
            .profiles
            .iter_mut()
            .find(|item| item.profile_id == profile.profile_id)
        {
            *existing = profile;
        } else {
            self.profiles.push(profile);
        }
        self.profiles
            .sort_by(|left, right| left.profile_id.cmp(&right.profile_id));
        Ok(())
    }

    pub fn activate(&mut self, profile_id: &str) -> Result<()> {
        if !self
            .profiles
            .iter()
            .any(|item| item.profile_id == profile_id)
        {
            bail!("provider profile '{profile_id}' does not exist");
        }
        self.active_profile = Some(profile_id.to_owned());
        Ok(())
    }

    pub fn active(&self) -> Result<&ProviderProfile> {
        let active_id = self
            .active_profile
            .as_deref()
            .context("no provider profile is active; add and activate one in Providers")?;
        self.profiles
            .iter()
            .find(|item| item.profile_id == active_id)
            .with_context(|| format!("active provider profile '{active_id}' is missing"))
    }

    pub fn get(&self, profile_id: &str) -> Option<&ProviderProfile> {
        self.profiles
            .iter()
            .find(|profile| profile.profile_id == profile_id)
    }

    pub fn remove(&mut self, profile_id: &str) -> Result<()> {
        let before = self.profiles.len();
        self.profiles
            .retain(|profile| profile.profile_id != profile_id);
        if self.profiles.len() == before {
            bail!("provider profile '{profile_id}' does not exist");
        }
        if self.active_profile.as_deref() == Some(profile_id) {
            self.active_profile = None;
        }
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        if self.version != SETTINGS_VERSION {
            bail!("unsupported provider settings version {}", self.version);
        }
        for profile in &self.profiles {
            profile.validate()?;
        }
        if let Some(active) = &self.active_profile {
            if !self
                .profiles
                .iter()
                .any(|profile| &profile.profile_id == active)
            {
                bail!("active provider profile '{active}' is missing");
            }
        }
        Ok(())
    }
}

pub fn settings_path() -> Result<PathBuf> {
    let root = crate::storage::root().context("resolving ~/.steward data root")?;
    Ok(root.join("providers.json"))
}

fn validate_identifier(label: &str, value: &str) -> Result<()> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    if !valid {
        bail!("{label} must use 1-64 ASCII letters, numbers, '-' or '_'");
    }
    Ok(())
}

fn validate_env_name(value: &str) -> Result<()> {
    let mut bytes = value.bytes();
    let first = bytes
        .next()
        .context("API key environment variable cannot be empty")?;
    if !(first.is_ascii_uppercase() || first == b'_')
        || !bytes.all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        bail!("API key environment variable must be uppercase ASCII with underscores");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ProviderProfile, ProviderProtocol, ProviderSettings};

    fn profile(id: &str) -> ProviderProfile {
        ProviderProfile {
            profile_id: id.to_owned(),
            provider_id: "openai".to_owned(),
            display_name: "OpenAI".to_owned(),
            protocol: ProviderProtocol::OpenAiChat,
            base_url: "https://api.openai.com/v1".to_owned(),
            model: "gpt-4o-mini".to_owned(),
            api_key_env: Some("OPENAI_API_KEY".to_owned()),
            api_key: None,
        }
    }

    #[test]
    fn settings_round_trip_when_profile_is_active() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let path = temp.path().join("providers.json");
        let mut settings = ProviderSettings::default();
        settings.upsert(profile("work")).expect("insert profile");
        settings.activate("work").expect("activate profile");
        settings.save(&path).expect("save settings");

        let loaded = ProviderSettings::load(&path).expect("load settings");

        assert_eq!(loaded.active().expect("active profile").profile_id, "work");
    }

    #[test]
    fn activation_fails_when_profile_is_unknown() {
        let mut settings = ProviderSettings::default();

        let error = settings.activate("missing").expect_err("unknown profile");

        assert!(error.to_string().contains("does not exist"));
    }

    #[test]
    fn removing_active_profile_clears_activation() {
        let mut settings = ProviderSettings::default();
        settings.upsert(profile("work")).expect("insert profile");
        settings.activate("work").expect("activate profile");

        settings.remove("work").expect("remove profile");

        assert!(settings.get("work").is_none());
        assert!(settings.active().is_err());
    }

    #[test]
    fn stored_api_key_survives_save_and_load_and_wins_over_env() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let path = temp.path().join("providers.json");
        let mut work_profile = profile("work");
        work_profile.api_key = Some("sk-stored-secret".to_owned());
        let mut settings = ProviderSettings::default();
        settings.upsert(work_profile).expect("insert profile");
        settings.save(&path).expect("save settings");

        let loaded = ProviderSettings::load(&path).expect("load settings");
        let loaded_profile = loaded.get("work").expect("profile present");

        assert_eq!(
            loaded_profile.api_key().expect("resolve api key"),
            Some("sk-stored-secret".to_owned())
        );
    }

    #[test]
    fn removing_unknown_profile_fails() {
        let mut settings = ProviderSettings::default();

        let error = settings.remove("missing").expect_err("unknown profile");

        assert!(error.to_string().contains("does not exist"));
    }
}
