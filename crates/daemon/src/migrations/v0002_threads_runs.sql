-- Omega §49 normative minimum: durable run substrate.
-- Indexes cover thread recency, run/thread, event ordering, node/run,
-- agent/run, and unresolved approvals.

CREATE TABLE IF NOT EXISTS threads (
    thread_id TEXT PRIMARY KEY,
    workspace_id TEXT,
    title TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    metadata_json TEXT NOT NULL DEFAULT '{}'
);

CREATE TABLE IF NOT EXISTS runs (
    run_id TEXT PRIMARY KEY,
    thread_id TEXT NOT NULL,
    parent_run_id TEXT,
    source_kind TEXT NOT NULL,
    status TEXT NOT NULL,
    execution_plan_json TEXT,
    success_policy_json TEXT NOT NULL,
    budget_json TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    started_at_ms INTEGER,
    completed_at_ms INTEGER,
    FOREIGN KEY(thread_id) REFERENCES threads(thread_id)
);

CREATE TABLE IF NOT EXISTS run_events (
    run_id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    timestamp_ms INTEGER NOT NULL,
    PRIMARY KEY(run_id, sequence)
);

CREATE TABLE IF NOT EXISTS checkpoints (
    checkpoint_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    parent_checkpoint_id TEXT,
    state_blob BLOB NOT NULL,
    created_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS node_attempts (
    attempt_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    node_id TEXT NOT NULL,
    attempt_number INTEGER NOT NULL,
    status TEXT NOT NULL,
    started_at_ms INTEGER NOT NULL,
    completed_at_ms INTEGER,
    outcome_json TEXT
);

CREATE TABLE IF NOT EXISTS agent_instances (
    agent_instance_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    parent_agent_instance_id TEXT,
    profile_id TEXT NOT NULL,
    status TEXT NOT NULL,
    budget_json TEXT NOT NULL,
    usage_json TEXT NOT NULL DEFAULT '{}',
    created_at_ms INTEGER NOT NULL,
    completed_at_ms INTEGER
);

CREATE TABLE IF NOT EXISTS approvals_v2 (
    approval_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    agent_instance_id TEXT,
    effect_json TEXT NOT NULL,
    scope_json TEXT NOT NULL,
    status TEXT NOT NULL,
    requested_at_ms INTEGER NOT NULL,
    resolved_at_ms INTEGER,
    resolution_json TEXT
);

CREATE INDEX IF NOT EXISTS idx_threads_updated ON threads(updated_at_ms);
CREATE INDEX IF NOT EXISTS idx_runs_thread ON runs(thread_id);
CREATE INDEX IF NOT EXISTS idx_run_events_ts ON run_events(timestamp_ms);
CREATE INDEX IF NOT EXISTS idx_node_attempts_run ON node_attempts(run_id);
CREATE INDEX IF NOT EXISTS idx_agent_instances_run ON agent_instances(run_id);
CREATE INDEX IF NOT EXISTS idx_approvals_unresolved ON approvals_v2(status)
    WHERE status = 'pending';
