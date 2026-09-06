use crate::cli::SetupArgs;
use anyhow::{bail, Context as _, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, IsTerminal as _, Write as _};
use std::path::{Path, PathBuf};
use steward_core::provider_catalog::{find as find_provider, PROVIDERS};
use steward_core::provider_config::{ProviderProfile, ProviderSettings};

const SETTINGS_VERSION: u32 = 1;
const DEFAULT_DAEMON_PORT: u16 = 50051;
const DEFAULT_WEB_PORT: u16 = 3000;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Settings {
    version: u32,
    pub web_port: u16,
}

impl Settings {
    pub fn new(web_port: u16) -> Result<Self> {
        if web_port == 0 {
            bail!("Web UI port must be between 1 and 65535");
        }
        if web_port == DEFAULT_DAEMON_PORT {
            bail!("daemon and Web UI ports must be different");
        }
        Ok(Self {
            version: SETTINGS_VERSION,
            web_port,
        })
    }

    pub fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)
            .with_context(|| format!("reading setup settings from {}", path.display()))?;
        let parsed: Self = serde_json::from_slice(&bytes)
            .with_context(|| format!("parsing setup settings from {}", path.display()))?;
        if parsed.version != SETTINGS_VERSION {
            bail!("unsupported setup settings version {}", parsed.version);
        }
        Self::new(parsed.web_port)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path.parent().context("setup settings path has no parent")?;
        fs::create_dir_all(parent)
            .with_context(|| format!("creating Steward data directory {}", parent.display()))?;
        let bytes = serde_json::to_vec_pretty(self)?;
        fs::write(path, bytes)
            .with_context(|| format!("writing setup settings to {}", path.display()))
    }

    pub fn daemon_url(&self) -> String {
        format!("http://127.0.0.1:{DEFAULT_DAEMON_PORT}")
    }

    pub fn web_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.web_port)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            web_port: DEFAULT_WEB_PORT,
        }
    }
}

pub fn settings_path() -> Result<PathBuf> {
    let root = steward_core::storage::root().context("resolving ~/.steward data root")?;
    Ok(root.join("setup.json"))
}

pub fn needs_setup(path: &Path) -> bool {
    Settings::load(path).is_err()
}

pub fn load_or_default(path: &Path) -> Result<Settings> {
    if path.exists() {
        Settings::load(path)
    } else {
        Ok(Settings::default())
    }
}

pub fn run(path: &Path, args: &SetupArgs) -> Result<Settings> {
    let current = if path.exists() {
        match Settings::load(path) {
            Ok(settings) => settings,
            Err(error) => {
                eprintln!("Existing setup settings are invalid: {error}");
                Settings::default()
            }
        }
    } else {
        Settings::default()
    };
    let selected = if args.quick {
        settings_from_args(args, &current)?
    } else {
        if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
            bail!("interactive setup requires a terminal; run `steward setup --quick`");
        }
        println!(
            "\nSTEWARD SETUP\nLocal services and operator console\n\n  1. QuickStart (recommended defaults)\n  2. Advanced (custom ports)\n"
        );
        print!("Setup mode [1]: ");
        io::stdout().flush()?;
        let mut choice = String::new();
        io::stdin().read_line(&mut choice)?;
        if matches!(choice.trim(), "" | "1") {
            Settings::default()
        } else if choice.trim() == "2" {
            let web_port = prompt_port("Web UI port", current.web_port)?;
            Settings::new(web_port)?
        } else {
            bail!("setup mode must be 1 or 2");
        }
    };
    selected.save(path)?;
    configure_provider(args)?;
    println!("Setup saved to {}", path.display());
    Ok(selected)
}

fn configure_provider(args: &SetupArgs) -> Result<()> {
    if args.skip_provider {
        return Ok(());
    }
    let provider_id = match args.provider.as_deref() {
        Some(provider) => provider.to_owned(),
        None if args.quick => return Ok(()),
        None => prompt_provider()?,
    };
    if provider_id.is_empty() {
        println!("Provider setup skipped. Run `steward setup` when ready.");
        return Ok(());
    }
    let provider =
        find_provider(&provider_id).with_context(|| format!("unknown provider '{provider_id}'"))?;
    let model = match args.model.as_deref() {
        Some(model) => model.trim().to_owned(),
        None if args.quick => bail!("--model is required when --provider is used with --quick"),
        None => prompt_required("Model ID")?,
    };
    let api_key_env = match (&args.api_key_env, provider.default_api_key_env) {
        (Some(value), _) if value.is_empty() => None,
        (Some(value), _) => Some(value.clone()),
        (None, Some(default)) if !args.quick => {
            let value = prompt_text("API key environment variable", default)?;
            (!value.is_empty()).then_some(value)
        }
        (None, default) => default.map(str::to_owned),
    };
    let profile = ProviderProfile {
        profile_id: "default".to_owned(),
        provider_id: provider.id.to_owned(),
        display_name: provider.name.to_owned(),
        protocol: provider.protocol,
        base_url: args
            .base_url
            .clone()
            .unwrap_or_else(|| provider.default_base_url.to_owned()),
        model,
        api_key_env,
        secret_ref: None,
        legacy_api_key: None,
    };
    save_provider_profile(&steward_core::provider_config::settings_path()?, profile)
}

fn save_provider_profile(path: &Path, profile: ProviderProfile) -> Result<()> {
    let mut settings = ProviderSettings::load(path)?;
    let profile_id = profile.profile_id.clone();
    settings.upsert(profile)?;
    settings.activate(&profile_id)?;
    settings.save(path)?;
    let active = settings.active()?;
    println!("Active model: {} / {}", active.display_name, active.model);
    if let Some(variable) = &active.api_key_env {
        if std::env::var_os(variable).is_none() {
            println!("Set {variable} before sending model requests.");
        }
    }
    Ok(())
}

fn prompt_provider() -> Result<String> {
    println!("\nMODEL PROVIDER\nChoose a provider or configure it later:\n");
    for (index, provider) in PROVIDERS.iter().enumerate() {
        println!("  {:>2}. {}", index + 1, provider.name);
    }
    println!("   0. Configure later");
    print!("Provider [0]: ");
    io::stdout().flush()?;
    let mut choice = String::new();
    io::stdin().read_line(&mut choice)?;
    let choice = choice.trim();
    if choice.is_empty() || choice == "0" {
        return Ok(String::new());
    }
    let index = choice
        .parse::<usize>()
        .context("provider choice must be a number")?;
    PROVIDERS
        .get(index.saturating_sub(1))
        .map(|provider| provider.id.to_owned())
        .context("provider choice is out of range")
}

fn prompt_required(label: &str) -> Result<String> {
    let value = prompt_text(label, "")?;
    if value.is_empty() {
        bail!("{label} cannot be empty");
    }
    Ok(value)
}

fn prompt_text(label: &str, default: &str) -> Result<String> {
    if default.is_empty() {
        print!("{label}: ");
    } else {
        print!("{label} [{default}]: ");
    }
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    let value = value.trim();
    Ok(if value.is_empty() {
        default.to_owned()
    } else {
        value.to_owned()
    })
}

fn prompt_port(label: &str, default: u16) -> Result<u16> {
    print!("{label} [{default}]: ");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    let value = value.trim();
    if value.is_empty() {
        Ok(default)
    } else {
        value
            .parse::<u16>()
            .with_context(|| format!("{label} must be between 1 and 65535"))
    }
}

fn settings_from_args(args: &SetupArgs, current: &Settings) -> Result<Settings> {
    Settings::new(args.web_port.unwrap_or(current.web_port))
}

#[cfg(test)]
#[path = "setup_tests.rs"]
mod tests;
