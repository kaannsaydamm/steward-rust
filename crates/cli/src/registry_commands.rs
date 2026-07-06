use crate::cli::{SkillCommand, ToolCommand, ToolHistoryArgs, ToolIdArgs, ToolInvokeArgs};
use crate::{client, registry_view};
use anyhow::{bail, Result};
use std::collections::BTreeMap;

pub async fn run_tools(host: &str, command: ToolCommand) -> Result<()> {
    match command {
        ToolCommand::List => {
            for tool in client::list_tools(host).await? {
                println!("{}", registry_view::tool_line(&tool));
            }
        }
        ToolCommand::Invoke(args) => invoke(host, args).await?,
        ToolCommand::History(args) => history(host, args).await?,
        ToolCommand::Enable(args) => set_enabled(host, args, true).await?,
        ToolCommand::Disable(args) => set_enabled(host, args, false).await?,
    }
    Ok(())
}

async fn set_enabled(host: &str, args: ToolIdArgs, enabled: bool) -> Result<()> {
    let tool = client::set_tool_enabled(host, &args.tool_id, enabled).await?;
    println!("{}", registry_view::tool_line(&tool));
    Ok(())
}

pub async fn run_skills(host: &str, command: SkillCommand) -> Result<()> {
    match command {
        SkillCommand::List => {
            for skill in client::list_skills(host).await? {
                println!("{}", registry_view::skill_line(&skill));
            }
        }
        SkillCommand::Install(args) => {
            let bundle = std::fs::read(&args.bundle)?;
            let skill = client::install_skill(host, bundle).await?;
            println!("installed\t{}\tversion={}", skill.skill_id, skill.version);
        }
    }
    Ok(())
}

async fn invoke(host: &str, args: ToolInvokeArgs) -> Result<()> {
    let arguments = parse_arguments(&args.arguments)?;
    let response = client::invoke_tool(host, &args.tool_id, arguments, args.approve).await?;
    println!("{}", registry_view::invocation_response_line(&response));
    if !response.output.is_empty() {
        println!("{}", response.output);
    }
    Ok(())
}

async fn history(host: &str, args: ToolHistoryArgs) -> Result<()> {
    for invocation in client::list_tool_invocations(host, args.limit.clamp(1, 100)).await? {
        println!("{}", registry_view::invocation_line(&invocation));
    }
    Ok(())
}

pub fn parse_arguments(raw: &[String]) -> Result<BTreeMap<String, String>> {
    let mut arguments = BTreeMap::new();
    for argument in raw {
        let Some((key, value)) = argument.split_once('=') else {
            bail!("tool argument must use KEY=VALUE: {argument}");
        };
        if key.trim().is_empty() {
            bail!("tool argument key cannot be empty");
        }
        if arguments.insert(key.to_owned(), value.to_owned()).is_some() {
            bail!("duplicate tool argument: {key}");
        }
    }
    Ok(arguments)
}

#[cfg(test)]
#[path = "registry_commands_tests.rs"]
mod tests;
