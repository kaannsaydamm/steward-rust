use anyhow::Result;
use chrono::{Local, Timelike};
use log::{info, warn};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use steward_knowledge::KnowledgeEngine;

pub struct DaemonConfig {
    pub addr: SocketAddr,
    pub web_addr: SocketAddr,
    pub dream_now: bool,
    pub dream_dir: PathBuf,
}

pub fn daemon_config() -> Result<DaemonConfig> {
    let mut port = 50051_u16;
    let mut web_port = 3000_u16;
    let mut dream_now = false;
    let mut dream_dir = steward_core::storage::root()
        .unwrap_or_else(|| PathBuf::from(".steward"))
        .join("memory/nightly");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => {
                if let Some(value) = args.next() {
                    port = value.parse()?;
                }
            }
            "--dream-now" => dream_now = true,
            "--web-port" => {
                if let Some(value) = args.next() {
                    web_port = value.parse()?;
                }
            }
            "--dream-dir" => {
                if let Some(value) = args.next() {
                    dream_dir = PathBuf::from(value);
                }
            }
            _ => {}
        }
    }
    Ok(DaemonConfig {
        addr: SocketAddr::from(([127, 0, 0, 1], port)),
        web_addr: SocketAddr::from(([127, 0, 0, 1], web_port)),
        dream_now,
        dream_dir,
    })
}

pub fn spawn_nightly_dream_scheduler(knowledge: Arc<KnowledgeEngine>, dream_dir: PathBuf) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(duration_until_next_midnight()).await;
            match create_nightly_dream_file(&knowledge, &dream_dir) {
                Ok(path) => info!("Nightly dream written: {}", path.display()),
                Err(error) => warn!("Nightly dream failed: {error}"),
            }
        }
    });
}

pub fn create_nightly_dream_file(knowledge: &KnowledgeEngine, dream_dir: &Path) -> Result<PathBuf> {
    let now = Local::now();
    let dream_date = now.date_naive().to_string();
    let end = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs_f64();
    let start = end - 86_400.0;
    let report = knowledge
        .memory
        .create_nightly_dream(&dream_date, start, end)?;
    std::fs::create_dir_all(dream_dir)?;
    let path = dream_dir.join(format!("{dream_date}.md"));
    std::fs::write(
        &path,
        format!(
            "# Steward Nightly Dream - {dream_date}\n\n{}\n\n- positive memories: {}\n- negative lessons: {}\n- reasoning traces: {}\n- memory id: {}\n",
            report.summary,
            report.positive_count,
            report.negative_count,
            report.reasoning_count,
            report.memory_id
        ),
    )?;
    Ok(path)
}

fn duration_until_next_midnight() -> Duration {
    let now = Local::now();
    let elapsed =
        u64::from(now.hour()) * 3600 + u64::from(now.minute()) * 60 + u64::from(now.second());
    Duration::from_secs((86_400 - elapsed).max(1))
}
