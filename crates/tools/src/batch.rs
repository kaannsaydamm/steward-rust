//! Parallel tool batch execution (§23.6, Task 7.6).
//!
//! Parallelize only independent calls whose effect scopes do not conflict;
//! serialize writes to the same file/workspace lease; preserve deterministic
//! result ordering (output order == input order).

use crate::effects::Effect;
use crate::spec::ToolSpec;
use anyhow::Result;
use serde_json::Value;
use std::collections::BTreeMap;
use std::future::Future;

/// One planned batch entry.
#[derive(Clone, Debug)]
pub struct BatchEntry {
    pub call_id: String,
    pub tool: ToolSpec,
    pub arguments: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BatchGroup {
    /// Safe to run concurrently with other Parallel entries.
    Parallel,
    /// Must run alone, in order (write/write or read-your-writes).
    Serialized,
}

/// Partitions a batch into groups.
pub fn partition(entries: &[BatchEntry]) -> Vec<(BatchGroup, Vec<usize>)> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mode {
        ReadOnly,
        Mutating,
    }
    let modes: Vec<Mode> = entries
        .iter()
        .map(|e| {
            if e.tool.effects.iter().any(|eff| {
                matches!(
                    eff,
                    Effect::FilesystemWrite
                        | Effect::FilesystemDelete
                        | Effect::GitWrite
                        | Effect::MemoryWrite
                        | Effect::WorkspaceMerge
                )
            }) {
                Mode::Mutating
            } else {
                Mode::ReadOnly
            }
        })
        .collect();

    // Group consecutive runs of read-only calls; mutating calls serialize.
    // (Across a model turn, same-file writes must keep order; simpler and
    // safer: any mutating call splits the batch.)
    let mut groups: Vec<(BatchGroup, Vec<usize>)> = Vec::new();
    let mut current: Option<(BatchGroup, Vec<usize>)> = None;
    for (index, mode) in modes.iter().enumerate() {
        let group = match mode {
            Mode::ReadOnly => BatchGroup::Parallel,
            Mode::Mutating => BatchGroup::Serialized,
        };
        match &mut current {
            Some((existing, members)) if *existing == group && group == BatchGroup::Parallel => {
                members.push(index);
            }
            _ => {
                if let Some(existing) = current.take() {
                    groups.push(existing);
                }
                current = Some((group, vec![index]));
            }
        }
    }
    if let Some(existing) = current.take() {
        groups.push(existing);
    }
    groups
}

/// Executes a batch: read-only runs run concurrently, mutating runs
/// sequentially, and results keep input order.
pub async fn execute_batch<F, Fut>(
    entries: Vec<BatchEntry>,
    mut exec: F,
) -> Result<Vec<(String, String)>>
where
    F: FnMut(BatchEntry) -> Fut,
    Fut: Future<Output = Result<(String, String)>>,
{
    let groups = partition(&entries);
    let mut results: Vec<Option<(String, String)>> = vec![None; entries.len()];
    let mut entry_iter = entries.into_iter();

    for (group, indexes) in groups {
        match group {
            BatchGroup::Parallel => {
                let mut futures = Vec::new();
                let mut positions = Vec::new();
                for index in indexes {
                    let entry = entry_iter.next().expect("batch entry");
                    positions.push(index);
                    futures.push(exec(entry));
                }
                let outputs = futures_util::future::join_all(futures).await;
                for (position, output) in positions.into_iter().zip(outputs) {
                    results[position] = Some(output?);
                }
            }
            BatchGroup::Serialized => {
                for index in indexes {
                    let entry = entry_iter.next().expect("batch entry");
                    let output = exec(entry).await?;
                    results[index] = Some(output);
                }
            }
        }
    }

    Ok(results
        .into_iter()
        .enumerate()
        .map(|(index, slot)| {
            slot.unwrap_or_else(|| (format!("call_{index}"), "missing result".to_owned()))
        })
        .collect())
}

/// Computes conflicting resources for callers that need lease checks.
pub fn resource_key(entry: &BatchEntry) -> BTreeMap<String, String> {
    let mut keys = BTreeMap::new();
    if let Some(path) = entry.arguments.get("path").and_then(Value::as_str) {
        keys.insert("path".into(), path.to_owned());
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;
    use std::time::Duration;

    fn tool(effects: Vec<Effect>) -> ToolSpec {
        ToolSpec {
            id: "t".into(),
            title: "t".into(),
            description: String::new(),
            schema: json!({}),
            effects,
            risk: crate::spec::RiskLevel::Low,
            tags: vec![],
            provider: "builtin".into(),
        }
    }

    fn read_entry(id: &str) -> BatchEntry {
        BatchEntry {
            call_id: id.into(),
            tool: tool(vec![Effect::FilesystemRead]),
            arguments: json!({}),
        }
    }

    fn write_entry(id: &str) -> BatchEntry {
        BatchEntry {
            call_id: id.into(),
            tool: tool(vec![Effect::FilesystemWrite]),
            arguments: json!({"path": "same.rs"}),
        }
    }

    #[test]
    fn read_only_entries_group_parallel() {
        let entries = vec![read_entry("a"), read_entry("b"), read_entry("c")];
        let groups = partition(&entries);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, BatchGroup::Parallel);
        assert_eq!(groups[0].1, vec![0, 1, 2]);
    }

    #[test]
    fn mutating_entries_split_the_batch() {
        let entries = vec![read_entry("a"), write_entry("w1"), read_entry("b")];
        let groups = partition(&entries);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].0, BatchGroup::Parallel);
        assert_eq!(groups[1].0, BatchGroup::Serialized);
        assert_eq!(groups[2].0, BatchGroup::Parallel);
    }

    #[tokio::test]
    async fn two_reads_run_concurrently() {
        use std::sync::Arc;
        use std::time::{Duration, Instant};

        struct Probe {
            concurrent: std::sync::atomic::AtomicIsize,
            peak: std::sync::atomic::AtomicIsize,
        }
        let probe = Arc::new(Probe {
            concurrent: std::sync::atomic::AtomicIsize::new(0),
            peak: std::sync::atomic::AtomicIsize::new(0),
        });
        let entries = vec![read_entry("a"), read_entry("b")];
        let started = Instant::now();
        let probe_clone = probe.clone();
        let results = execute_batch(entries, move |entry| {
            let probe = probe_clone.clone();
            async move {
                let now = probe
                    .concurrent
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                    + 1;
                probe
                    .peak
                    .fetch_max(now, std::sync::atomic::Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(50)).await;
                probe
                    .concurrent
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                Ok((entry.call_id, "ok".into()))
            }
        })
        .await
        .unwrap();
        let elapsed = started.elapsed();
        assert_eq!(results.len(), 2);
        assert!(
            probe.peak.load(std::sync::atomic::Ordering::SeqCst) >= 2,
            "reads overlap"
        );
        assert!(
            elapsed < Duration::from_millis(90),
            "concurrent, not serial: {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn overlapping_writes_serialize_in_order() {
        use parking_lot::Mutex as PLMutex;
        let order = Arc::new(PLMutex::new(Vec::new()));
        let entries = vec![write_entry("w1"), write_entry("w2"), write_entry("w3")];
        let order_clone = order.clone();
        let results = execute_batch(entries, move |entry| {
            let order = order_clone.clone();
            async move {
                order.lock().push(entry.call_id.clone());
                Ok((entry.call_id, "ok".into()))
            }
        })
        .await
        .unwrap();
        assert_eq!(
            *order.lock(),
            vec!["w1".to_owned(), "w2".to_owned(), "w3".to_owned()],
            "writes keep submission order"
        );
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn result_order_matches_input_order() {
        let entries = vec![read_entry("a"), read_entry("b"), read_entry("c")];
        let results = execute_batch(entries, |entry| async move {
            // Sleep longest for the first call to prove ordering is by input.
            let delay = match entry.call_id.as_str() {
                "a" => 60,
                "b" => 30,
                _ => 10,
            };
            tokio::time::sleep(Duration::from_millis(delay)).await;
            Ok((entry.call_id, "ok".into()))
        })
        .await
        .unwrap();
        let ids: Vec<&str> = results.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn resource_key_extracts_path() {
        let entry = BatchEntry {
            call_id: "x".into(),
            tool: tool(vec![Effect::FilesystemWrite]),
            arguments: json!({"path": "src/lib.rs"}),
        };
        assert_eq!(
            resource_key(&entry).get("path").map(String::as_str),
            Some("src/lib.rs")
        );
    }
}
