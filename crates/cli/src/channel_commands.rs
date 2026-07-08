//! `steward channel ...` — configure the Telegram bridge. These commands edit
//! `~/.steward/channels.json` directly; the daemon reads the token at startup (restart to
//! start/stop the bridge) but re-reads the chat allowlist on every message, so allow/disallow
//! take effect immediately.

use crate::cli::ChannelCommand;
use anyhow::Result;
use steward_core::channel_settings::{settings_path, ChannelSettings};

pub fn run(command: ChannelCommand) -> Result<()> {
    let path = settings_path()?;
    let mut settings = ChannelSettings::load(&path)?;
    match command {
        ChannelCommand::Status => {
            println!(
                "telegram_token: {}",
                if settings.telegram_token.is_some() {
                    "configured"
                } else {
                    "not set"
                }
            );
            if settings.telegram_allowed_chats.is_empty() {
                println!("allowed chats: none (bridge only replies with pairing instructions)");
            } else {
                for chat in &settings.telegram_allowed_chats {
                    println!("allowed chat: {chat}");
                }
            }
        }
        ChannelCommand::SetTelegram(args) => {
            settings.telegram_token = Some(args.token);
            settings.save(&path)?;
            println!("telegram token saved; restart the daemon to start the bridge");
        }
        ChannelCommand::ClearTelegram => {
            settings.telegram_token = None;
            settings.save(&path)?;
            println!("telegram token removed; restart the daemon to stop the bridge");
        }
        ChannelCommand::Allow(args) => {
            settings.allow_chat(args.chat_id);
            settings.save(&path)?;
            println!("allowed chat {}", args.chat_id);
        }
        ChannelCommand::Disallow(args) => {
            settings.disallow_chat(args.chat_id);
            settings.save(&path)?;
            println!("disallowed chat {}", args.chat_id);
        }
    }
    Ok(())
}
