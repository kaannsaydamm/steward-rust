use super::{CheckLevel, DiagnosticCheck, DoctorReport, EndpointScope};

#[test]
fn endpoint_scope_distinguishes_local_and_remote_hosts() {
    assert_eq!(
        EndpointScope::parse("http://127.0.0.1:50051"),
        EndpointScope::Local { port: 50051 }
    );
    assert_eq!(
        EndpointScope::parse("http://localhost:37241/"),
        EndpointScope::Local { port: 37241 }
    );
    assert_eq!(
        EndpointScope::parse("https://steward.example.com:443"),
        EndpointScope::Remote
    );
    assert_eq!(
        EndpointScope::parse("not-an-endpoint"),
        EndpointScope::Invalid
    );
}

#[test]
fn report_lines_include_each_check_and_failure_summary() {
    let report = DoctorReport::new(vec![
        DiagnosticCheck::pass("cli", "version 0.1.0"),
        DiagnosticCheck::warn("database", "not created yet"),
        DiagnosticCheck::fail("daemon", "connection refused"),
    ]);

    assert_eq!(
        report.lines(),
        vec![
            "[pass] cli: version 0.1.0",
            "[warn] database: not created yet",
            "[fail] daemon: connection refused",
            "summary: 1 passed, 1 warning, 1 failed",
        ]
    );
    assert!(!report.is_healthy());
    assert_eq!(report.checks()[2].level(), CheckLevel::Fail);
}

#[test]
fn warnings_do_not_make_a_report_unhealthy() {
    let report = DoctorReport::new(vec![DiagnosticCheck::warn(
        "nightly memory",
        "directory will be created on first dream",
    )]);

    assert!(report.is_healthy());
    assert_eq!(
        report.lines().last().map(String::as_str),
        Some("summary: 0 passed, 1 warning, 0 failed")
    );
}
