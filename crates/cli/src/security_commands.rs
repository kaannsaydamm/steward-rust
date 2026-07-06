use crate::cli::SecurityCommand;
use crate::client;
use anyhow::Result;

pub async fn run(host: &str, command: SecurityCommand) -> Result<()> {
    match command {
        SecurityCommand::List => {
            let settings = client::get_security_settings(host).await?;
            if settings.process_exec_allowlist.is_empty() {
                println!("no allowlist configured; process.exec permits any command once approved");
            }
            for program in settings.process_exec_allowlist {
                println!("{program}");
            }
        }
        SecurityCommand::Allow(args) => {
            let mut settings = client::get_security_settings(host).await?;
            if !settings.process_exec_allowlist.contains(&args.program) {
                settings.process_exec_allowlist.push(args.program.clone());
                settings.process_exec_allowlist.sort();
            }
            client::save_security_settings(host, settings.process_exec_allowlist).await?;
            println!("allowed\t{}", args.program);
        }
        SecurityCommand::Disallow(args) => {
            let mut settings = client::get_security_settings(host).await?;
            settings
                .process_exec_allowlist
                .retain(|program| program != &args.program);
            client::save_security_settings(host, settings.process_exec_allowlist).await?;
            println!("disallowed\t{}", args.program);
        }
    }
    Ok(())
}
