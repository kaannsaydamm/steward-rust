use super::*;

fn setup() -> Connection {
    let connection = Connection::open_in_memory().expect("open database");
    create_schema(&connection).expect("create schema");
    connection
}

#[test]
fn create_job_rejects_short_interval() {
    let connection = setup();
    let error = create_job(&connection, "ping", "memory.recall", "{}", 10).unwrap_err();
    assert!(error.to_string().contains("interval_seconds"));
}

#[test]
fn create_job_rejects_invalid_json() {
    let connection = setup();
    let error = create_job(&connection, "ping", "memory.recall", "{not json", 300).unwrap_err();
    assert!(error.to_string().contains("valid JSON"));
}

#[test]
fn create_and_list_round_trips() {
    let connection = setup();
    let job =
        create_job(&connection, "Nightly prune", "memory.recall", "{}", 3600).expect("create job");
    let jobs = list_jobs(&connection).expect("list jobs");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].job_id, job.job_id);
    assert!(jobs[0].enabled);
    assert_eq!(jobs[0].last_run_at, 0.0);
}

#[test]
fn set_enabled_toggles_flag() {
    let connection = setup();
    let job = create_job(&connection, "job", "memory.recall", "{}", 300).expect("create job");
    let updated = set_enabled(&connection, &job.job_id, false).expect("disable job");
    assert!(!updated.enabled);
}

#[test]
fn set_enabled_missing_job_errors() {
    let connection = setup();
    let error = set_enabled(&connection, "missing", true).unwrap_err();
    assert!(error.to_string().contains("does not exist"));
}

#[test]
fn delete_job_removes_row() {
    let connection = setup();
    let job = create_job(&connection, "job", "memory.recall", "{}", 300).expect("create job");
    assert!(delete_job(&connection, &job.job_id).expect("delete"));
    assert!(get_job(&connection, &job.job_id)
        .expect("get job")
        .is_none());
}

#[test]
fn due_jobs_respects_interval_and_enabled_flag() {
    let connection = setup();
    let due = create_job(&connection, "due", "memory.recall", "{}", 60).expect("create due");
    let not_due =
        create_job(&connection, "not-due", "memory.recall", "{}", 3600).expect("create not due");
    let disabled =
        create_job(&connection, "disabled", "memory.recall", "{}", 60).expect("create disabled");
    set_enabled(&connection, &disabled.job_id, false).expect("disable");
    mark_run(&connection, &not_due.job_id, "succeeded").expect("mark not_due run");

    let now = unix_seconds() + 120.0;
    let due_jobs = due_jobs(&connection, now).expect("due jobs");
    let due_ids: Vec<_> = due_jobs.iter().map(|job| job.job_id.clone()).collect();

    assert!(due_ids.contains(&due.job_id));
    assert!(!due_ids.contains(&not_due.job_id));
    assert!(!due_ids.contains(&disabled.job_id));
}

#[test]
fn mark_run_updates_status_and_timestamp() {
    let connection = setup();
    let job = create_job(&connection, "job", "memory.recall", "{}", 300).expect("create job");
    mark_run(&connection, &job.job_id, "succeeded").expect("mark run");
    let refreshed = get_job(&connection, &job.job_id)
        .expect("get job")
        .expect("job exists");
    assert_eq!(refreshed.last_status, "succeeded");
    assert!(refreshed.last_run_at > 0.0);
}
