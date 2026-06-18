use crate::cli::RegistryCommand;
use crate::{client, registry_view};
use anyhow::Result;

pub async fn run_tools(host: &str, command: RegistryCommand) -> Result<()> {
    match command {
        RegistryCommand::List => {
            for tool in client::list_tools(host).await? {
                println!("{}", registry_view::tool_line(&tool));
            }
        }
    }
    Ok(())
}

pub async fn run_skills(host: &str, command: RegistryCommand) -> Result<()> {
    match command {
        RegistryCommand::List => {
            for skill in client::list_skills(host).await? {
                println!("{}", registry_view::skill_line(&skill));
            }
        }
    }
    Ok(())
}
