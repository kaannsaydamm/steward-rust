use crate::client_chat;
use anyhow::{Context as _, Result};
use clap::{Args, Subcommand};
use steward_core::pb::ProviderProfileInfo;

#[derive(Debug, Args)]
pub struct ProviderArgs {
    #[command(subcommand)]
    pub command: ProviderCommand,
}

#[derive(Debug, Subcommand)]
pub enum ProviderCommand {
    Catalog,
    List,
    Add(ProviderAddArgs),
    Use(ProviderUseArgs),
    Remove(ProviderRemoveArgs),
}

#[derive(Debug, Args)]
pub struct ProviderRemoveArgs {
    pub profile_id: String,
}

#[derive(Debug, Args)]
pub struct ProviderAddArgs {
    pub provider_id: String,
    pub profile_id: String,
    #[arg(long)]
    pub model: String,
    #[arg(long)]
    pub base_url: Option<String>,
    #[arg(long)]
    pub api_key_env: Option<String>,
    /// Persists the key value directly on the profile (survives daemon restarts without
    /// needing the environment variable set). Prefer this over --api-key-env for convenience.
    #[arg(long)]
    pub api_key: Option<String>,
    #[arg(long, default_value_t = false)]
    pub activate: bool,
}

#[derive(Debug, Args)]
pub struct ProviderUseArgs {
    pub profile_id: String,
}

pub async fn run(host: &str, command: ProviderCommand) -> Result<()> {
    match command {
        ProviderCommand::Catalog => {
            for provider in client_chat::provider_catalog(host).await? {
                println!(
                    "{}\t{}\t{}\t{}",
                    provider.provider_id,
                    provider.name,
                    provider.default_base_url,
                    provider.default_api_key_env
                );
            }
        }
        ProviderCommand::List => {
            let profiles = client_chat::provider_profiles(host).await?;
            if profiles.is_empty() {
                println!("no provider profiles; run `steward provider add --help`");
            }
            for profile in profiles {
                let active = if profile.active { "*" } else { " " };
                println!(
                    "{active}\t{}\t{}\t{}\t{}",
                    profile.profile_id, profile.display_name, profile.model, profile.base_url
                );
            }
        }
        ProviderCommand::Add(args) => add(host, args).await?,
        ProviderCommand::Use(args) => {
            let profile = client_chat::activate_provider(host, &args.profile_id).await?;
            println!(
                "active provider: {} / {}",
                profile.profile_id, profile.model
            );
        }
        ProviderCommand::Remove(args) => {
            let removed = client_chat::delete_provider(host, &args.profile_id).await?;
            println!("removed={removed}\t{}", args.profile_id);
        }
    }
    Ok(())
}

async fn add(host: &str, args: ProviderAddArgs) -> Result<()> {
    let catalog = client_chat::provider_catalog(host).await?;
    let provider = catalog
        .into_iter()
        .find(|provider| provider.provider_id == args.provider_id)
        .with_context(|| format!("provider '{}' is not in the catalog", args.provider_id))?;
    let base_url = args.base_url.unwrap_or(provider.default_base_url);
    let api_key_env = args.api_key_env.unwrap_or(provider.default_api_key_env);
    let profile = client_chat::save_provider(
        host,
        ProviderProfileInfo {
            profile_id: args.profile_id,
            provider_id: provider.provider_id,
            display_name: provider.name,
            protocol: provider.protocol,
            base_url,
            model: args.model,
            api_key_env,
            active: false,
            api_key: args.api_key.unwrap_or_default(),
            has_stored_key: false,
        },
        args.activate,
    )
    .await?;
    println!(
        "saved provider: {} / {}{}",
        profile.profile_id,
        profile.model,
        if profile.active { " [active]" } else { "" }
    );
    Ok(())
}
