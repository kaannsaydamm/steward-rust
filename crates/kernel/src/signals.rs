//! Run signals / live steering (§32, Task 18.2, K-013/K-014/U-005).
//!
//! A running thread/run can receive signals from ANY client; they are
//! durable events (another client sees them). Steer applies at the next
//! safe turn boundary; pause/cancel act immediately at boundaries.

use anyhow::Result;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunSignal {
    /// Natural-language steering, applied at the next turn boundary (K-013).
    Steer {
        instruction: String,
    },
    /// Wake a waiting run; duplicate signal ids are idempotent (K-014).
    Wake {
        signal_id: String,
    },
    Pause,
    Resume,
    Cancel,
    RaisePriority,
    LowerPriority,
    AddContext {
        reference: String,
    },
    /// Adjust budget mid-run (K-016 companion).
    ChangeBudget {
        max_model_calls: Option<u32>,
        max_tool_calls: Option<u32>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignalEvent {
    pub signal: RunSignal,
    pub run_id: String,
    /// Ordered per run; duplicate Wake ids carry the same dedupe key.
    pub sequence: u64,
    pub source_client: String,
    pub timestamp_ms: i64,
}

/// Registry: ordered durable signals per run.
#[derive(Default)]
pub struct SignalBus {
    queues: Mutex<std::collections::BTreeMap<String, Vec<SignalEvent>>>,
    wake_dedupe: Mutex<std::collections::BTreeSet<String>>,
    counters: Mutex<std::collections::BTreeMap<String, u64>>,
}

impl SignalBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sends a signal from any client (second client steering the same run
    /// is the acceptance case, U-005).
    pub fn send(
        &self,
        run_id: &str,
        signal: RunSignal,
        source_client: &str,
    ) -> Result<SignalEvent> {
        // K-014: duplicate Wake signal ids are idempotent.
        if let RunSignal::Wake { signal_id } = &signal {
            let mut dedupe = self.wake_dedupe.lock();
            if !dedupe.insert(signal_id.clone()) {
                anyhow::bail!("duplicate wake signal '{signal_id}' ignored");
            }
        }
        let mut counters = self.counters.lock();
        let sequence = counters.entry(run_id.to_owned()).or_insert(0);
        *sequence += 1;
        let event = SignalEvent {
            signal,
            run_id: run_id.to_owned(),
            sequence: *sequence,
            source_client: source_client.to_owned(),
            timestamp_ms: now_ms(),
        };
        drop(counters);
        self.queues
            .lock()
            .entry(run_id.to_owned())
            .or_default()
            .push(event.clone());
        Ok(event)
    }

    /// Drains pending signals for a run (kernel applies at turn boundary).
    pub fn drain(&self, run_id: &str) -> Vec<SignalEvent> {
        self.queues.lock().remove(run_id).unwrap_or_default()
    }

    pub fn pending_count(&self, run_id: &str) -> usize {
        self.queues.lock().get(run_id).map(Vec::len).unwrap_or(0)
    }
}

/// The steering outcome a turn boundary computes from drained signals.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TurnAdjustments {
    pub steer_instructions: Vec<String>,
    pub pause_requested: bool,
    pub cancel_requested: bool,
    pub resume_requested: bool,
    pub context_additions: Vec<String>,
    pub budget_patch: Option<(Option<u32>, Option<u32>)>,
}

/// Applies drained signals to produce turn adjustments (K-013).
pub fn apply_at_boundary(events: Vec<SignalEvent>) -> TurnAdjustments {
    let mut adjustments = TurnAdjustments::default();
    for event in events {
        match event.signal {
            RunSignal::Steer { instruction } => adjustments.steer_instructions.push(instruction),
            RunSignal::Pause => adjustments.pause_requested = true,
            RunSignal::Resume => adjustments.resume_requested = true,
            RunSignal::Cancel => adjustments.cancel_requested = true,
            RunSignal::AddContext { reference } => adjustments.context_additions.push(reference),
            RunSignal::ChangeBudget {
                max_model_calls,
                max_tool_calls,
            } => {
                adjustments.budget_patch = Some((max_model_calls, max_tool_calls));
            }
            RunSignal::Wake { .. } | RunSignal::RaisePriority | RunSignal::LowerPriority => {}
        }
    }
    adjustments
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_client_steers_the_same_run() {
        let bus = SignalBus::new();
        // Client A started the run; client B sends steer.
        let event = bus
            .send(
                "run_1",
                RunSignal::Steer {
                    instruction: "focus on the auth module".into(),
                },
                "client-b",
            )
            .unwrap();
        assert_eq!(event.sequence, 1);
        assert_eq!(event.source_client, "client-b");

        let drained = bus.drain("run_1");
        assert_eq!(drained.len(), 1);
        assert_eq!(
            drained[0].signal,
            RunSignal::Steer {
                instruction: "focus on the auth module".into()
            }
        );
        assert_eq!(bus.pending_count("run_1"), 0);
    }

    #[test]
    fn signals_are_ordered_per_run() {
        let bus = SignalBus::new();
        bus.send("run_1", RunSignal::Pause, "a").unwrap();
        bus.send("run_1", RunSignal::Resume, "b").unwrap();
        bus.send("run_1", RunSignal::Cancel, "a").unwrap();

        let drained = bus.drain("run_1");
        assert_eq!(
            drained.iter().map(|e| e.sequence).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn wake_signals_are_idempotent_by_id() {
        let bus = SignalBus::new();
        bus.send(
            "run_1",
            RunSignal::Wake {
                signal_id: "w1".into(),
            },
            "a",
        )
        .unwrap();
        assert!(bus
            .send(
                "run_1",
                RunSignal::Wake {
                    signal_id: "w1".into()
                },
                "b"
            )
            .is_err());
        // Distinct id passes.
        assert!(bus
            .send(
                "run_1",
                RunSignal::Wake {
                    signal_id: "w2".into()
                },
                "a"
            )
            .is_ok());
    }

    #[test]
    fn boundary_application_maps_all_signal_kinds() {
        let events = vec![
            SignalEvent {
                signal: RunSignal::Steer {
                    instruction: "slow down".into(),
                },
                run_id: "r".into(),
                sequence: 1,
                source_client: "web".into(),
                timestamp_ms: 1,
            },
            SignalEvent {
                signal: RunSignal::Cancel,
                run_id: "r".into(),
                sequence: 2,
                source_client: "web".into(),
                timestamp_ms: 2,
            },
            SignalEvent {
                signal: RunSignal::AddContext {
                    reference: "file:notes.md".into(),
                },
                run_id: "r".into(),
                sequence: 3,
                source_client: "web".into(),
                timestamp_ms: 3,
            },
            SignalEvent {
                signal: RunSignal::ChangeBudget {
                    max_model_calls: Some(5),
                    max_tool_calls: None,
                },
                run_id: "r".into(),
                sequence: 4,
                source_client: "web".into(),
                timestamp_ms: 4,
            },
        ];
        let adjustments = apply_at_boundary(events);
        assert_eq!(adjustments.steer_instructions, vec!["slow down".to_owned()]);
        assert!(adjustments.cancel_requested);
        assert_eq!(
            adjustments.context_additions,
            vec!["file:notes.md".to_owned()]
        );
        assert_eq!(adjustments.budget_patch, Some((Some(5), None)));
    }

    #[test]
    fn signals_are_serializable_for_durability() {
        let event = SignalEvent {
            signal: RunSignal::Pause,
            run_id: "run_1".into(),
            sequence: 1,
            source_client: "cli".into(),
            timestamp_ms: 5,
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: SignalEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back, event);
    }
}
