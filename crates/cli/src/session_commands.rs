use crate::client_chat;
use anyhow::Result;
use clap::{Args, Subcommand};
use steward_core::pb::ChatEventKind;

#[derive(Debug, Args)]
pub struct SessionArgs {
    #[command(subcommand)]
    pub command: SessionCommand,
}

#[derive(Debug, Subcommand)]
pub enum SessionCommand {
    List(SessionListArgs),
    Show(SessionIdArgs),
    Delete(SessionIdArgs),
    Resume(SessionResumeArgs),
}

#[derive(Debug, Args)]
pub struct SessionListArgs {
    #[arg(long, default_value_t = 20)]
    pub limit: i32,
}

#[derive(Debug, Args)]
pub struct SessionIdArgs {
    pub session_id: String,
}

#[derive(Debug, Args)]
pub struct SessionResumeArgs {
    pub session_id: String,
    #[arg(required = true)]
    pub message: Vec<String>,
}

pub async fn run(host: &str, command: SessionCommand) -> Result<()> {
    match command {
        SessionCommand::List(args) => {
            for session in client_chat::sessions(host, args.limit).await? {
                println!(
                    "{}\t{}\t{}\t{}",
                    session.session_id, session.provider_profile, session.model, session.title
                );
            }
        }
        SessionCommand::Show(args) => {
            let session = client_chat::session(host, &args.session_id).await?;
            for message in session.messages {
                println!("{}\t{}", message.role, message.content.replace('\n', " "));
            }
        }
        SessionCommand::Delete(args) => {
            println!(
                "deleted={}",
                client_chat::delete_session(host, &args.session_id).await?
            );
        }
        SessionCommand::Resume(args) => {
            let events =
                client_chat::chat(host, &args.session_id, &args.message.join(" "), true).await?;
            for event in events {
                if event.kind == ChatEventKind::Text as i32 {
                    print!("{}", event.content);
                }
            }
            println!();
        }
    }
    Ok(())
}
