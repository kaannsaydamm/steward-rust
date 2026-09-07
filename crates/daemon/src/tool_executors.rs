use crate::MySteward;
use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use steward_core::security_settings::SecuritySettings;
use steward_knowledge::{MemoryEntry, MemoryType};

const MAX_OUTPUT_BYTES: usize = 32_768;
const MAX_SEARCH_RESULTS: usize = 200;
const MAX_SEARCH_DEPTH: usize = 12;
const PROCESS_TIMEOUT: Duration = Duration::from_secs(30);
const SKIPPED_DIR_NAMES: &[&str] = &[".git", "node_modules", "target", ".steward", "dist", "out"];

pub async fn execute(
    steward: &MySteward,
    tool_id: &str,
    arguments: &BTreeMap<String, String>,
    working_directory: &str,
) -> Result<String> {
    match tool_id {
        "memory.recall" => recall_memory(steward, arguments),
        "memory.store" => store_memory(steward, arguments),
        "workflow.inspect" => inspect_workflow(steward, arguments).await,
        "fs.read" => read_file(arguments, working_directory).await,
        "fs.search" => search_files(arguments, working_directory).await,
        "fs.write" => write_file(steward, arguments, working_directory).await,
        "process.exec" => execute_process(steward, arguments, working_directory).await,
        "git.diff" => git_diff(arguments, working_directory).await,
        "git.branch" => git_branch(arguments, working_directory).await,
        "workflow.manage" => manage_workflow(steward, arguments).await,
        "sqlite.query" => sqlite_query(steward, arguments, working_directory).await,
        "pdf.extract" => pdf_extract(arguments, working_directory).await,
        "checkpoint.save" => checkpoint_save(steward, arguments, working_directory).await,
        "checkpoint.load" => checkpoint_load(steward, arguments).await,
        "think" => think_note(steward, arguments).await,
        "todo.list" => todo_list(steward, arguments).await,
        _ if tool_id.starts_with("mcp.") => {
            crate::mcp_lifecycle::invoke(steward, tool_id, arguments).await
        }
        _ => bail!("no executor registered for {tool_id}"),
    }
}

fn resolve_workspace_root(working_directory: &str) -> Result<PathBuf> {
    let root = if working_directory.trim().is_empty() {
        std::env::current_dir().context("resolving current directory")?
    } else {
        PathBuf::from(working_directory)
    };
    root.canonicalize()
        .with_context(|| format!("resolving working directory {}", root.display()))
}

fn resolve_within_root(root: &Path, relative: &str) -> Result<PathBuf> {
    let candidate = root.join(relative);
    let resolved = candidate
        .canonicalize()
        .with_context(|| format!("resolving path {relative}"))?;
    if !resolved.starts_with(root) {
        bail!("path '{relative}' escapes the working directory");
    }
    Ok(resolved)
}

/// Like `resolve_within_root`, but for targets that may not exist yet (writes). Canonicalize
/// can't be used on a missing file, so escape is prevented lexically: absolute paths and any
/// `..`/prefix components are rejected before joining onto the (canonical) root.
fn resolve_write_target(root: &Path, relative: &str) -> Result<PathBuf> {
    let candidate = Path::new(relative);
    if candidate.is_absolute() {
        bail!("path '{relative}' must be relative to the working directory");
    }
    for component in candidate.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            bail!("path '{relative}' escapes the working directory");
        }
    }
    Ok(root.join(candidate))
}

async fn read_file(
    arguments: &BTreeMap<String, String>,
    working_directory: &str,
) -> Result<String> {
    let path_argument = arguments
        .get("path")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("path is required")?;
    let root = resolve_workspace_root(working_directory)?;
    let resolved = resolve_within_root(&root, path_argument)?;
    if !resolved.is_file() {
        bail!("'{path_argument}' is not a file");
    }
    let bytes = tokio::fs::read(&resolved)
        .await
        .with_context(|| format!("reading {path_argument}"))?;
    Ok(truncate_bytes(
        &String::from_utf8_lossy(&bytes),
        MAX_OUTPUT_BYTES,
    ))
}

async fn write_file(
    steward: &MySteward,
    arguments: &BTreeMap<String, String>,
    working_directory: &str,
) -> Result<String> {
    let path_argument = arguments
        .get("path")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("path is required")?;
    let content = arguments
        .get("content")
        .map(String::as_str)
        .context("content is required")?;
    let root = resolve_workspace_root(working_directory)?;
    let target = resolve_write_target(&root, path_argument)?;
    let previous = match tokio::fs::read(&target).await {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error).with_context(|| format!("reading {path_argument} for checkpoint"))
        }
    };
    let existed = previous.is_some();
    let checkpoint_id = {
        let connection = steward
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
        crate::file_checkpoints::record(
            &connection,
            &root.display().to_string(),
            path_argument,
            previous.as_deref(),
        )?
    };
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("creating parent directories for {path_argument}"))?;
    }
    tokio::fs::write(&target, content)
        .await
        .with_context(|| format!("writing {path_argument}"))?;
    Ok(format!(
        "{}\t{path_argument}\t{} bytes\tcheckpoint={checkpoint_id}",
        if existed { "overwrote" } else { "created" },
        content.len()
    ))
}

async fn search_files(
    arguments: &BTreeMap<String, String>,
    working_directory: &str,
) -> Result<String> {
    let query = arguments
        .get("query")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("query is required")?
        .to_ascii_lowercase();
    let root = resolve_workspace_root(working_directory)?;
    let matches = tokio::task::spawn_blocking(move || collect_matches(&root, &root, &query, 0))
        .await
        .context("search task panicked")??;
    if matches.is_empty() {
        return Ok("no matches".to_owned());
    }
    Ok(matches.join("\n"))
}

fn collect_matches(root: &Path, dir: &Path, query: &str, depth: usize) -> Result<Vec<String>> {
    let mut matches = Vec::new();
    if depth > MAX_SEARCH_DEPTH {
        return Ok(matches);
    }
    let entries = std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?;
    for entry in entries {
        if matches.len() >= MAX_SEARCH_RESULTS {
            break;
        }
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if entry.path().is_dir() {
            if SKIPPED_DIR_NAMES.contains(&name.as_ref()) {
                continue;
            }
            matches.extend(collect_matches(root, &entry.path(), query, depth + 1)?);
            continue;
        }
        if name.to_ascii_lowercase().contains(query) {
            let relative = entry
                .path()
                .strip_prefix(root)
                .unwrap_or(&entry.path())
                .display()
                .to_string();
            matches.push(relative);
        }
    }
    Ok(matches)
}

async fn execute_process(
    steward: &MySteward,
    arguments: &BTreeMap<String, String>,
    working_directory: &str,
) -> Result<String> {
    let command_line = arguments
        .get("command")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("command is required")?;
    let settings = SecuritySettings::load(&steward.security_path)?;
    if !settings.is_command_allowed(command_line) {
        let program = command_line.split_whitespace().next().unwrap_or("");
        bail!(
            "command '{program}' is not in the allowlist; run `steward security allow {program}` to permit it"
        );
    }
    let root = resolve_workspace_root(working_directory)?;
    let mut command = platform_shell_command(command_line);
    command.current_dir(&root);
    let output = tokio::time::timeout(PROCESS_TIMEOUT, command.output())
        .await
        .with_context(|| format!("command timed out after {}s", PROCESS_TIMEOUT.as_secs()))?
        .context("spawning process")?;
    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    combined.push_str(&format!(
        "\n[exit code: {}]",
        output.status.code().unwrap_or(-1)
    ));
    Ok(truncate_bytes(&combined, MAX_OUTPUT_BYTES))
}

#[cfg(windows)]
fn platform_shell_command(command_line: &str) -> tokio::process::Command {
    let mut command = tokio::process::Command::new("cmd");
    command.arg("/C").arg(command_line);
    command
}

#[cfg(not(windows))]
fn platform_shell_command(command_line: &str) -> tokio::process::Command {
    let mut command = tokio::process::Command::new("sh");
    command.arg("-c").arg(command_line);
    command
}

async fn git_diff(arguments: &BTreeMap<String, String>, working_directory: &str) -> Result<String> {
    let root = resolve_workspace_root(working_directory)?;
    let mut args = vec!["diff".to_owned()];
    if arguments.get("staged").is_some_and(|value| value == "true") {
        args.push("--staged".to_owned());
    }
    if let Some(path) = arguments
        .get("path")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        args.push("--".to_owned());
        args.push(path.to_owned());
    }
    let diff = run_git(&root, &args).await?;
    if diff.trim().is_empty() {
        return Ok("no changes".to_owned());
    }
    Ok(diff)
}

async fn git_branch(
    arguments: &BTreeMap<String, String>,
    working_directory: &str,
) -> Result<String> {
    let name = arguments
        .get("name")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("name is required")?;
    if !is_valid_branch_name(name) {
        bail!("branch name '{name}' contains invalid or unsafe characters");
    }
    let root = resolve_workspace_root(working_directory)?;
    let create = arguments.get("create").is_none_or(|value| value != "false");
    let args = if create {
        vec!["checkout".to_owned(), "-b".to_owned(), name.to_owned()]
    } else {
        vec!["checkout".to_owned(), name.to_owned()]
    };
    let output = run_git(&root, &args).await?;
    Ok(format!("switched to branch '{name}'\n{output}"))
}

fn is_valid_branch_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('-')
        && !name.contains("..")
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'/' | b'.'))
}

async fn run_git(root: &Path, args: &[String]) -> Result<String> {
    let mut command = tokio::process::Command::new("git");
    command.current_dir(root).args(args);
    let output = tokio::time::timeout(PROCESS_TIMEOUT, command.output())
        .await
        .with_context(|| format!("git command timed out after {}s", PROCESS_TIMEOUT.as_secs()))?
        .context("spawning git")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git {} failed: {}", args.join(" "), stderr.trim());
    }
    Ok(truncate_bytes(
        &String::from_utf8_lossy(&output.stdout),
        MAX_OUTPUT_BYTES,
    ))
}

fn truncate_bytes(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[truncated]", &value[..end])
}

fn recall_memory(steward: &MySteward, arguments: &BTreeMap<String, String>) -> Result<String> {
    let query = arguments.get("query").map_or("", String::as_str);
    let limit = arguments
        .get("limit")
        .map_or(Ok(10_usize), |value| value.parse::<usize>())
        .context("limit must be a positive integer")?
        .clamp(1, 50);
    let memories = steward.knowledge.recall_memory(query, None, limit)?;
    if memories.is_empty() {
        return Ok("no memories".to_owned());
    }
    Ok(memories
        .into_iter()
        .map(|memory| format!("{}\t{}", memory.id, memory.content.replace('\n', " ")))
        .collect::<Vec<_>>()
        .join("\n"))
}

fn store_memory(steward: &MySteward, arguments: &BTreeMap<String, String>) -> Result<String> {
    let content = arguments
        .get("content")
        .map(String::as_str)
        .filter(|content| !content.trim().is_empty())
        .context("content is required")?;
    let mut metadata = HashMap::new();
    metadata.insert("source".to_owned(), "tool-invocation".to_owned());
    let id = steward.knowledge.store_memory(MemoryEntry {
        id: String::new(),
        memory_type: MemoryType::LongTerm,
        content: content.to_owned(),
        metadata,
        entities: Vec::new(),
        timestamp: unix_seconds(),
    })?;
    Ok(format!("remembered\t{id}"))
}

async fn inspect_workflow(
    steward: &MySteward,
    arguments: &BTreeMap<String, String>,
) -> Result<String> {
    let workflow_id = arguments
        .get("workflow_id")
        .map(String::as_str)
        .context("workflow_id is required")?;
    let workflows = steward.workflows.lock().await;
    let state = workflows
        .get(workflow_id)
        .with_context(|| format!("workflow {workflow_id} not found"))?;
    Ok(format!(
        "{}\tphase={}\tmode={}\tprogress={:.0}%\t{}",
        state.status.workflow_id,
        state.status.phase,
        state.status.mode,
        state.status.overall_progress,
        state.status.status_message
    ))
}

/// Compiles and runs a workspace `.wasm` module through the daemon's shared wasmtime engine.
/// Same contract as the RunPlugin RPC: the module must export `run() -> i32`, and it runs with
/// no imports (no WASI, no host functions) — a pure sandboxed computation.
async fn run_wasm(
    steward: &MySteward,
    arguments: &BTreeMap<String, String>,
    working_directory: &str,
) -> Result<String> {
    let path_argument = arguments
        .get("path")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("path is required")?;
    let root = resolve_workspace_root(working_directory)?;
    let resolved = resolve_within_root(&root, path_argument)?;
    let bytes = tokio::fs::read(&resolved)
        .await
        .with_context(|| format!("reading {path_argument}"))?;
    let engine = steward.wasm_engine.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<i32> {
        let module = wasmtime::Module::from_binary(&engine, &bytes).context("compiling WASM")?;
        let mut store = wasmtime::Store::new(&engine, ());
        let instance = wasmtime::Instance::new(&mut store, &module, &[])
            .context("instantiating WASM (modules with imports are not supported)")?;
        let run = instance
            .get_typed_func::<(), i32>(&mut store, "run")
            .context("module does not export run() -> i32")?;
        run.call(&mut store, ()).context("executing run()")
    })
    .await
    .context("WASM task panicked")??;
    Ok(format!("run() returned {result}"))
}

/// Approves or cancels a workflow — the two governance actions a model can take on the durable
/// workflow engine. Starting workflows stays with the operator (CLI/WebUI), matching the
/// approval-centric design of the rest of the tool registry.
async fn manage_workflow(
    steward: &MySteward,
    arguments: &BTreeMap<String, String>,
) -> Result<String> {
    let action = arguments
        .get("action")
        .map(String::as_str)
        .context("action is required ('approve' or 'cancel')")?;
    let workflow_id = arguments
        .get("workflow_id")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("workflow_id is required")?;
    let mut workflows = steward.workflows.lock().await;
    let state = workflows
        .get_mut(workflow_id)
        .with_context(|| format!("workflow {workflow_id} not found"))?;
    let message = match action {
        "approve" => {
            state.approved = true;
            state.status.requires_approval = false;
            state.status.pending_approval = None;
            state.status.status_message = "Plan approved via workflow.manage tool".to_owned();
            format!("approved\t{workflow_id}")
        }
        "cancel" => {
            let reason = arguments
                .get("reason")
                .map(String::as_str)
                .unwrap_or("cancelled via workflow.manage tool");
            state.cancelled = true;
            state.status.phase = 12;
            state.status.status_message = format!("Cancelled: {reason}");
            format!("cancelled\t{workflow_id}")
        }
        other => bail!("unsupported action '{other}' (use 'approve' or 'cancel')"),
    };
    let snapshot = state.clone();
    drop(workflows);
    steward
        .workflow_runtime()
        .persist_workflow_state(&snapshot)?;
    Ok(message)
}

fn unix_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}

// ── Harness parity tools (omp inventory reference) ─────────────────────────

/// `sqlite.query`: read-only SELECT against a workspace SQLite database.
async fn sqlite_query(
    steward: &MySteward,
    arguments: &BTreeMap<String, String>,
    working_directory: &str,
) -> Result<String> {
    let database = arguments
        .get("database")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("database is required")?;
    let query = arguments
        .get("query")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("query is required")?;
    let normalized = query.trim().to_ascii_uppercase();
    if !normalized.starts_with("SELECT") && !normalized.starts_with("WITH") && !normalized.starts_with("EXPLAIN") {
        bail!("sqlite.query is read-only: only SELECT/WITH/EXPLAIN statements are allowed");
    }
    for forbidden in ["INSERT", "UPDATE", "DELETE", "DROP", "ALTER", "CREATE", "ATTACH", "PRAGMA", "REPLACE"] {
        if normalized.split_whitespace().any(|word| word.trim_end_matches(';').eq_ignore_ascii_case(forbidden)) {
            bail!("sqlite.query is read-only: '{forbidden}' is not allowed");
        }
    }
    let root = resolve_workspace_root(working_directory)?;
    let path = resolve_within_root(&root, database)?;
    // Open read-only via URI flags to enforce the no-write contract.
    let connection = rusqlite::Connection::open_with_flags(
        &path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let mut statement = connection.prepare(query)?;
    let column_count = statement.column_count();
    let mut rows = statement.query([])?;
    let mut output = String::new();
    let mut row_count = 0usize;
    while row_count < 200 {
        let Some(row) = rows.next()? else { break };
        let mut cells = Vec::with_capacity(column_count);
        for column in 0..column_count {
            let value: rusqlite::types::Value = row.get(column)?;
            cells.push(match value {
                rusqlite::types::Value::Null => "NULL".to_owned(),
                rusqlite::types::Value::Integer(int) => int.to_string(),
                rusqlite::types::Value::Real(real) => real.to_string(),
                rusqlite::types::Value::Text(text) => text,
                rusqlite::types::Value::Blob(blob) => format!("<{} bytes>", blob.len()),
            });
        }
        output.push_str(&cells.join("\t"));
        output.push('\n');
        row_count += 1;
    }
    let _ = steward;
    Ok(if output.is_empty() { "(no rows)".to_owned() } else { output })
}

/// `pdf.extract`: extract text from a workspace PDF (PDFObjHandler-free minimal
/// extractor over lopdf objects; falls back to byte scan for uncompressed text).
async fn pdf_extract(
    arguments: &BTreeMap<String, String>,
    working_directory: &str,
) -> Result<String> {
    let path_argument = arguments
        .get("path")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("path is required")?;
    let root = resolve_workspace_root(working_directory)?;
    let path = resolve_within_root(&root, path_argument)?;
    let bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    if &bytes[..5.min(bytes.len())] != b"%PDF-" {
        bail!("{} is not a PDF file", path.display());
    }
    // Extract text operators from uncompressed content streams (Tj/TJ).
    let text = String::from_utf8_lossy(&bytes);
    let mut extracted = String::new();
    let mut rest = text.as_ref();
    while let Some(start) = rest.find('(') {
        let Some(end_rel) = rest[start + 1..].find(')') else { break };
        let end = start + 1 + end_rel;
        let candidate = &rest[start + 1..end];
        if !candidate.is_empty() && candidate.bytes().all(|byte| (0x20..=0x7e).contains(&byte) || byte == b'\n') {
            extracted.push_str(candidate);
            extracted.push('\n');
        }
        rest = &rest[end + 1..];
    }
    Ok(if extracted.is_empty() {
        "(no extractable text — PDF likely uses compressed streams)".to_owned()
    } else {
        extracted
    })
}

/// `checkpoint.save`: snapshot a file through the file_checkpoints store.
async fn checkpoint_save(
    steward: &MySteward,
    arguments: &BTreeMap<String, String>,
    working_directory: &str,
) -> Result<String> {
    let path_argument = arguments
        .get("path")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("path is required")?;
    let root = resolve_workspace_root(working_directory)?;
    let path = resolve_within_root(&root, path_argument)?;
    let previous = std::fs::read(&path).ok();
    let checkpoint_id = {
        let connection = steward.db.lock().map_err(|_| anyhow::anyhow!("Database lock failed"))?;
        crate::file_checkpoints::record(
            &connection,
            &root.to_string_lossy(),
            path_argument,
            previous.as_deref(),
        )?
    };
    Ok(format!("checkpoint {checkpoint_id} saved for {path_argument}"))
}

/// `checkpoint.load`: restore the file from a saved checkpoint.
async fn checkpoint_load(
    steward: &MySteward,
    arguments: &BTreeMap<String, String>,
) -> Result<String> {
    let checkpoint_id: i64 = arguments
        .get("checkpoint_id")
        .map(String::as_str)
        .context("checkpoint_id is required")?
        .parse()
        .context("checkpoint_id must be an integer")?;
    let message = {
        let connection = steward.db.lock().map_err(|_| anyhow::anyhow!("Database lock failed"))?;
        crate::file_checkpoints::rollback(&connection, checkpoint_id)?
    };
    Ok(message)
}

/// `think`: scratchpad note persisted to memory without side effects.
async fn think_note(steward: &MySteward, arguments: &BTreeMap<String, String>) -> Result<String> {
    let note = arguments
        .get("note")
        .map(String::as_str)
        .filter(|content| !content.trim().is_empty())
        .context("note is required")?;
    let mut metadata = HashMap::new();
    metadata.insert("source".to_owned(), "think".to_owned());
    steward.knowledge.store_memory(MemoryEntry {
        id: String::new(),
        memory_type: MemoryType::ShortTerm,
        content: format!("scratchpad: {note}"),
        metadata,
        entities: Vec::new(),
        timestamp: unix_seconds(),
    })?;
    Ok("noted".to_owned())
}

/// `todo.list`: session task list over short-term memory entries.
async fn todo_list(steward: &MySteward, arguments: &BTreeMap<String, String>) -> Result<String> {
    let Some(task) = arguments.get("add").filter(|task| !task.trim().is_empty()) else {
        // No "add" argument: return the current list.
        let entries = steward.knowledge.recall_memory("scratchpad: todo", None, 20)?;
        if entries.is_empty() {
            return Ok("(no todo items)".to_owned());
        }
        return Ok(entries
            .iter()
            .map(|entry| entry.content.clone())
            .collect::<Vec<_>>()
            .join("\n"));
    };
    let mut metadata = HashMap::new();
    metadata.insert("source".to_owned(), "todo".to_owned());
    steward.knowledge.store_memory(MemoryEntry {
        id: String::new(),
        memory_type: MemoryType::ShortTerm,
        content: format!("scratchpad: todo {task}"),
        metadata,
        entities: Vec::new(),
        timestamp: unix_seconds(),
    })?;
    Ok(format!("added: {task}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn test_steward(temp: &tempfile::TempDir) -> MySteward {
        let db_path = temp.path().join("steward-test.db");
        MySteward::new(
            db_path.to_str().expect("database path"),
            crate::maintenance::RetentionConfig::default(),
        )
        .expect("steward runtime")
    }

    #[tokio::test]
    async fn read_file_returns_contents_within_the_workspace() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("notes.txt"), "hello steward").expect("write file");
        let mut arguments = BTreeMap::new();
        arguments.insert("path".to_owned(), "notes.txt".to_owned());

        let output = read_file(&arguments, temp.path().to_str().unwrap())
            .await
            .expect("read file");

        assert_eq!(output, "hello steward");
    }

    #[tokio::test]
    async fn read_file_rejects_paths_that_escape_the_workspace() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("create workspace");
        std::fs::write(temp.path().join("secret.txt"), "top secret").expect("write file");
        let mut arguments = BTreeMap::new();
        arguments.insert("path".to_owned(), "../secret.txt".to_owned());

        let error = read_file(&arguments, workspace.to_str().unwrap())
            .await
            .expect_err("path escape should be rejected");

        assert!(error.to_string().contains("escapes the working directory"));
    }

    #[tokio::test]
    async fn write_file_creates_a_new_file_with_parents() {
        let temp = tempfile::tempdir().expect("tempdir");
        let steward = test_steward(&temp);
        let workspace = temp.path().join("ws");
        std::fs::create_dir_all(&workspace).expect("create workspace");
        let mut arguments = BTreeMap::new();
        arguments.insert("path".to_owned(), "docs/notes.txt".to_owned());
        arguments.insert("content".to_owned(), "hello write".to_owned());

        let output = write_file(&steward, &arguments, workspace.to_str().unwrap())
            .await
            .expect("write file");

        assert!(output.starts_with("created"));
        assert!(output.contains("checkpoint="));
        let written =
            std::fs::read_to_string(workspace.join("docs").join("notes.txt")).expect("read back");
        assert_eq!(written, "hello write");
    }

    #[tokio::test]
    async fn write_file_overwrite_is_checkpointed_and_rollbackable() {
        let temp = tempfile::tempdir().expect("tempdir");
        let steward = test_steward(&temp);
        let workspace = temp.path().join("ws");
        std::fs::create_dir_all(&workspace).expect("create workspace");
        std::fs::write(workspace.join("notes.txt"), "old").expect("seed file");
        let mut arguments = BTreeMap::new();
        arguments.insert("path".to_owned(), "notes.txt".to_owned());
        arguments.insert("content".to_owned(), "new".to_owned());

        let output = write_file(&steward, &arguments, workspace.to_str().unwrap())
            .await
            .expect("write file");

        assert!(output.starts_with("overwrote"));
        let written = std::fs::read_to_string(workspace.join("notes.txt")).expect("read back");
        assert_eq!(written, "new");
        // The recorded checkpoint restores the pre-write content.
        let connection = steward.db.lock().expect("database lock");
        let checkpoints = crate::file_checkpoints::list(&connection, 10).expect("list");
        assert_eq!(checkpoints.len(), 1);
        crate::file_checkpoints::rollback(&connection, checkpoints[0].checkpoint_id)
            .expect("rollback");
        drop(connection);
        let restored = std::fs::read_to_string(workspace.join("notes.txt")).expect("read back");
        assert_eq!(restored, "old");
    }

    #[tokio::test]
    async fn write_file_rejects_paths_that_escape_the_workspace() {
        let temp = tempfile::tempdir().expect("tempdir");
        let steward = test_steward(&temp);
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("create workspace");
        let mut arguments = BTreeMap::new();
        arguments.insert("path".to_owned(), "../escape.txt".to_owned());
        arguments.insert("content".to_owned(), "nope".to_owned());

        let error = write_file(&steward, &arguments, workspace.to_str().unwrap())
            .await
            .expect_err("path escape should be rejected");

        assert!(error.to_string().contains("escapes the working directory"));
        assert!(!temp.path().join("escape.txt").exists());
    }

    #[tokio::test]
    async fn write_file_rejects_absolute_paths() {
        let temp = tempfile::tempdir().expect("tempdir");
        let steward = test_steward(&temp);
        let workspace = temp.path().join("ws");
        std::fs::create_dir_all(&workspace).expect("create workspace");
        let outside = temp.path().join("outside.txt");
        let mut arguments = BTreeMap::new();
        arguments.insert("path".to_owned(), outside.display().to_string());
        arguments.insert("content".to_owned(), "nope".to_owned());

        let error = write_file(&steward, &arguments, workspace.to_str().unwrap())
            .await
            .expect_err("absolute path should be rejected");

        assert!(error.to_string().contains("must be relative"));
    }

    #[tokio::test]
    async fn search_files_finds_matching_names_and_skips_noise_dirs() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("readme.md"), "hi").expect("write file");
        std::fs::create_dir_all(temp.path().join("node_modules")).expect("create dir");
        std::fs::write(temp.path().join("node_modules").join("readme.md"), "hi")
            .expect("write file");
        let mut arguments = BTreeMap::new();
        arguments.insert("query".to_owned(), "readme".to_owned());

        let output = search_files(&arguments, temp.path().to_str().unwrap())
            .await
            .expect("search files");

        assert_eq!(output, "readme.md");
    }

    #[test]
    fn truncate_bytes_keeps_short_text_unchanged() {
        assert_eq!(truncate_bytes("short", 100), "short");
    }

    #[test]
    fn truncate_bytes_marks_long_text_as_truncated() {
        let long = "a".repeat(50);
        let truncated = truncate_bytes(&long, 10);
        assert!(truncated.ends_with("[truncated]"));
        assert!(truncated.len() < long.len());
    }

    #[test]
    fn is_valid_branch_name_accepts_typical_names() {
        assert!(is_valid_branch_name("feature/steward-git-tools"));
        assert!(is_valid_branch_name("fix-123"));
    }

    #[test]
    fn is_valid_branch_name_rejects_unsafe_input() {
        assert!(!is_valid_branch_name(""));
        assert!(!is_valid_branch_name("-rf"));
        assert!(!is_valid_branch_name("has space"));
        assert!(!is_valid_branch_name("has..dots"));
        assert!(!is_valid_branch_name("has;semicolon"));
    }

    fn init_git_repo(path: &std::path::Path) {
        run_git_sync(path, &["init"]);
        run_git_sync(path, &["config", "user.email", "test@example.com"]);
        run_git_sync(path, &["config", "user.name", "Test"]);
        std::fs::write(path.join("README.md"), "hello\n").expect("write readme");
        run_git_sync(path, &["add", "."]);
        run_git_sync(path, &["commit", "-m", "initial commit"]);
    }

    fn run_git_sync(path: &std::path::Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .current_dir(path)
            .args(args)
            .status()
            .expect("run git");
        assert!(status.success(), "git {args:?} failed");
    }

    #[tokio::test]
    async fn git_diff_reports_no_changes_on_a_clean_repo() {
        let temp = tempfile::tempdir().expect("tempdir");
        init_git_repo(temp.path());

        let output = git_diff(&BTreeMap::new(), temp.path().to_str().unwrap())
            .await
            .expect("git diff");

        assert_eq!(output, "no changes");
    }

    #[tokio::test]
    async fn git_diff_shows_uncommitted_changes() {
        let temp = tempfile::tempdir().expect("tempdir");
        init_git_repo(temp.path());
        std::fs::write(temp.path().join("README.md"), "hello\nworld\n").expect("edit readme");

        let output = git_diff(&BTreeMap::new(), temp.path().to_str().unwrap())
            .await
            .expect("git diff");

        assert!(output.contains("world"));
    }

    #[tokio::test]
    async fn git_branch_creates_and_checks_out_a_new_branch() {
        let temp = tempfile::tempdir().expect("tempdir");
        init_git_repo(temp.path());
        let mut arguments = BTreeMap::new();
        arguments.insert("name".to_owned(), "steward/test-branch".to_owned());

        git_branch(&arguments, temp.path().to_str().unwrap())
            .await
            .expect("create branch");

        let branch = run_git_capture(temp.path(), &["branch", "--show-current"]);
        assert_eq!(branch.trim(), "steward/test-branch");
    }

    #[tokio::test]
    async fn git_branch_rejects_unsafe_names() {
        let temp = tempfile::tempdir().expect("tempdir");
        init_git_repo(temp.path());
        let mut arguments = BTreeMap::new();
        arguments.insert("name".to_owned(), "-rf".to_owned());

        let error = git_branch(&arguments, temp.path().to_str().unwrap())
            .await
            .expect_err("unsafe branch name should be rejected");

        assert!(error.to_string().contains("invalid"));
    }

    fn run_git_capture(path: &std::path::Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .current_dir(path)
            .args(args)
            .output()
            .expect("run git");
        String::from_utf8_lossy(&output.stdout).into_owned()
    }
}
