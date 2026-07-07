use crate::cli::CronCommand;
use crate::client;
use anyhow::Result;

pub async fn run(host: &str, command: CronCommand) -> Result<()> {
    match command {
        CronCommand::List => {
            let jobs = client::list_cron_jobs(host).await?;
            if jobs.is_empty() {
                println!("no cron jobs configured");
            }
            for job in jobs {
                println!(
                    "{}\t{}\t{}\tevery {}s\t{}\tlast: {}",
                    job.job_id,
                    job.name,
                    job.tool_id,
                    job.interval_seconds,
                    if job.enabled { "enabled" } else { "disabled" },
                    if job.last_status.is_empty() {
                        "never run"
                    } else {
                        &job.last_status
                    },
                );
            }
        }
        CronCommand::Create(args) => {
            let job = client::create_cron_job(
                host,
                &args.name,
                &args.tool_id,
                &args.input_json,
                args.interval_seconds,
            )
            .await?;
            println!("created\t{}\t{}", job.job_id, job.name);
        }
        CronCommand::Enable(args) => {
            let job = client::set_cron_job_enabled(host, &args.job_id, true).await?;
            println!("enabled\t{}", job.job_id);
        }
        CronCommand::Disable(args) => {
            let job = client::set_cron_job_enabled(host, &args.job_id, false).await?;
            println!("disabled\t{}", job.job_id);
        }
        CronCommand::Delete(args) => {
            let deleted = client::delete_cron_job(host, &args.job_id).await?;
            println!(
                "{}\t{}",
                if deleted { "deleted" } else { "not found" },
                args.job_id
            );
        }
        CronCommand::Run(args) => {
            let job = client::run_cron_job_now(host, &args.job_id).await?;
            println!("ran\t{}\t{}", job.job_id, job.last_status);
        }
    }
    Ok(())
}
