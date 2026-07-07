use crate::cli::DashboardArgs;
use crate::daemon_lifecycle;
use anyhow::Result;

pub async fn run(host: &str, web_url: &str, web_port: u16, args: DashboardArgs) -> Result<()> {
    if args.stop {
        let stopped = daemon_lifecycle::stop_local_daemon()?;
        println!(
            "{}",
            if stopped {
                "Stopped the local Steward daemon."
            } else {
                "No locally-started Steward daemon was tracked (nothing to stop)."
            }
        );
        return Ok(());
    }

    if args.status {
        match daemon_lifecycle::ping_with_timeout(host).await {
            Ok(status) => println!("Daemon: online ({status})\nWeb UI: {web_url}"),
            Err(_) => println!("Daemon: offline"),
        }
        return Ok(());
    }

    daemon_lifecycle::ensure_running(host, true, web_port).await?;
    daemon_lifecycle::ensure_web_ready(web_port).await?;
    daemon_lifecycle::open_browser(web_url)?;
    println!("Web UI: {web_url}");
    Ok(())
}
