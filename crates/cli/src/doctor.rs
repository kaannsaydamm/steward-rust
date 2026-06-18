use crate::{client, daemon_lifecycle};
use anyhow::{bail, Result};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckLevel {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Eq, PartialEq)]
pub struct DiagnosticCheck {
    level: CheckLevel,
    name: &'static str,
    detail: String,
}

impl DiagnosticCheck {
    pub fn pass(name: &'static str, detail: impl Into<String>) -> Self {
        Self::new(CheckLevel::Pass, name, detail)
    }

    pub fn warn(name: &'static str, detail: impl Into<String>) -> Self {
        Self::new(CheckLevel::Warn, name, detail)
    }

    pub fn fail(name: &'static str, detail: impl Into<String>) -> Self {
        Self::new(CheckLevel::Fail, name, detail)
    }

    #[cfg(test)]
    pub const fn level(&self) -> CheckLevel {
        self.level
    }

    fn new(level: CheckLevel, name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            level,
            name,
            detail: detail.into(),
        }
    }

    fn line(&self) -> String {
        let level = match self.level {
            CheckLevel::Pass => "pass",
            CheckLevel::Warn => "warn",
            CheckLevel::Fail => "fail",
        };
        format!("[{level}] {}: {}", self.name, self.detail)
    }
}

#[derive(Debug)]
pub struct DoctorReport {
    checks: Vec<DiagnosticCheck>,
}

impl DoctorReport {
    pub const fn new(checks: Vec<DiagnosticCheck>) -> Self {
        Self { checks }
    }

    #[cfg(test)]
    pub fn checks(&self) -> &[DiagnosticCheck] {
        &self.checks
    }

    pub fn is_healthy(&self) -> bool {
        !self
            .checks
            .iter()
            .any(|check| check.level == CheckLevel::Fail)
    }

    pub fn lines(&self) -> Vec<String> {
        let mut passed = 0;
        let mut warnings = 0;
        let mut failed = 0;
        let mut lines = Vec::with_capacity(self.checks.len() + 1);
        for check in &self.checks {
            match check.level {
                CheckLevel::Pass => passed += 1,
                CheckLevel::Warn => warnings += 1,
                CheckLevel::Fail => failed += 1,
            }
            lines.push(check.line());
        }
        lines.push(format!(
            "summary: {passed} passed, {warnings} warning, {failed} failed"
        ));
        lines
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EndpointScope {
    Local { port: u16 },
    Remote,
    Invalid,
}

impl EndpointScope {
    pub fn parse(host: &str) -> Self {
        let Some(authority) = host
            .strip_prefix("http://")
            .or_else(|| host.strip_prefix("https://"))
        else {
            return Self::Invalid;
        };
        let authority = authority.strip_suffix('/').unwrap_or(authority);
        let Some((name, port)) = authority.rsplit_once(':') else {
            return Self::Invalid;
        };
        let Ok(port) = port.parse::<u16>() else {
            return Self::Invalid;
        };
        if matches!(name, "127.0.0.1" | "localhost" | "[::1]") {
            Self::Local { port }
        } else {
            Self::Remote
        }
    }
}

pub async fn collect(host: &str, auto_start: bool) -> DoctorReport {
    let mut checks = vec![DiagnosticCheck::pass(
        "cli",
        format!("steward {}", env!("CARGO_PKG_VERSION")),
    )];
    let scope = EndpointScope::parse(host);
    checks.push(endpoint_check(host, scope, auto_start));

    if matches!(scope, EndpointScope::Local { .. }) {
        let daemon = daemon_lifecycle::daemon_path();
        checks.push(path_check("daemon binary", daemon, true));
    }

    let startup_error = if auto_start && matches!(scope, EndpointScope::Local { .. }) {
        daemon_lifecycle::ensure_running(host, true)
            .await
            .err()
            .map(|error| format!("{error:#}"))
    } else {
        None
    };
    match client::ping(host).await {
        Ok(status) => checks.push(DiagnosticCheck::pass("daemon", status)),
        Err(error) => checks.push(DiagnosticCheck::fail(
            "daemon",
            startup_error.unwrap_or_else(|| format!("{error:#}")),
        )),
    }

    let root = steward_core::storage::root().unwrap_or_else(|| PathBuf::from(".steward"));
    checks.push(path_check("database", root.join("steward.db"), false));
    checks.push(path_check(
        "nightly memory",
        root.join("memory").join("nightly"),
        false,
    ));
    DoctorReport::new(checks)
}

pub async fn run(host: &str, auto_start: bool, strict: bool) -> Result<()> {
    let report = collect(host, auto_start).await;
    for line in report.lines() {
        println!("{line}");
    }
    if strict && !report.is_healthy() {
        bail!("steward doctor found failing checks");
    }
    Ok(())
}

fn endpoint_check(host: &str, scope: EndpointScope, auto_start: bool) -> DiagnosticCheck {
    match scope {
        EndpointScope::Local { port } => DiagnosticCheck::pass(
            "endpoint",
            format!("local port {port}; auto-start={auto_start}"),
        ),
        EndpointScope::Remote => DiagnosticCheck::pass("endpoint", format!("remote {host}")),
        EndpointScope::Invalid => DiagnosticCheck::fail("endpoint", format!("invalid {host}")),
    }
}

fn path_check(name: &'static str, path: PathBuf, required: bool) -> DiagnosticCheck {
    if path.exists() {
        DiagnosticCheck::pass(name, path.display().to_string())
    } else if required {
        DiagnosticCheck::fail(name, format!("missing {}", path.display()))
    } else {
        DiagnosticCheck::warn(name, format!("not created at {}", path.display()))
    }
}

#[cfg(test)]
#[path = "doctor_tests.rs"]
mod tests;
