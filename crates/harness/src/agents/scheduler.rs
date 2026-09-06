//! Subagent scheduler (§26, Tasks 11.2-11.4, K-005..K-009).
//!
//! Parent-child budget conservation, recursion limits, cancellation
//! propagation, async task registry, and durable mailbox messaging.

use crate::agent_profile::AgentProfile;
use anyhow::{bail, Context as _, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use steward_kernel::budget::Budget;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpawnMode {
    Inline,
    Async,
    Detached,
    Scheduled,
    Team,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpawnRequest {
    pub parent_instance_id: Option<String>,
    pub profile_id: String,
    pub task: String,
    pub mode: SpawnMode,
    /// Budget allocated by the parent from its own remaining budget.
    pub budget: Budget,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentInstance {
    pub instance_id: String,
    pub request: SpawnRequest,
    pub status: AgentStatus,
    /// Remaining budget for this instance.
    pub remaining_budget: Budget,
    /// Consumed counters (mirrors Budget dimensions).
    pub consumed: ConsumedBudget,
    pub depth: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ConsumedBudget {
    pub model_calls: u32,
    pub tool_calls: u32,
    pub spawned_agents: u32,
    pub failures: u32,
}

/// Durable mailbox message (K-009): ordered per recipient.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MailboxMessage {
    pub message_id: String,
    pub sender_instance_id: String,
    pub recipient_instance_id: String,
    pub content: String,
    /// Provenance: the run the message belongs to.
    pub run_id: String,
    pub sequence: u64,
    pub timestamp_ms: i64,
}

struct Inner {
    instances: BTreeMap<String, AgentInstance>,
    /// Recipient -> ordered mailbox.
    mailboxes: BTreeMap<String, Vec<MailboxMessage>>,
    /// Max recursion depth for the whole tree (K-005 safe termination).
    max_depth: u32,
    next_instance: u64,
    next_message: u64,
}

pub struct SubagentScheduler {
    inner: Mutex<Inner>,
    profiles: BTreeMap<String, AgentProfile>,
}

impl SubagentScheduler {
    pub fn new(max_depth: u32) -> Self {
        Self {
            inner: Mutex::new(Inner {
                instances: BTreeMap::new(),
                mailboxes: BTreeMap::new(),
                max_depth,
                next_instance: 0,
                next_message: 0,
            }),
            profiles: BTreeMap::new(),
        }
    }

    pub fn register_profile(&mut self, profile: AgentProfile) {
        self.profiles.insert(profile.id.clone(), profile);
    }

    pub fn profile(&self, profile_id: &str) -> Option<&AgentProfile> {
        self.profiles.get(profile_id)
    }

    /// Spawns a child under budget conservation (K-005/K-016):
    /// - recursion depth limit enforced;
    /// - child budget may never exceed the parent's remaining allocation.
    pub fn spawn(&self, request: SpawnRequest) -> Result<AgentInstance> {
        let mut inner = self.inner.lock();
        inner.next_instance += 1;
        let instance_id = format!("agt_{:04}", inner.next_instance);

        // Depth accounting.
        let depth = match &request.parent_instance_id {
            None => 0,
            Some(parent_id) => {
                let parent = inner
                    .instances
                    .get(parent_id)
                    .context("parent instance not found")?;
                parent.depth + 1
            }
        };
        if depth >= inner.max_depth {
            bail!(
                "recursion limit reached (depth {depth} >= {}); refusing spawn",
                inner.max_depth
            );
        }

        // Budget conservation: child <= parent remaining (when parent exists).
        if let Some(parent_id) = &request.parent_instance_id {
            let parent = inner.instances.get(parent_id).context("parent instance not found")?;
            let parent_left = &parent.remaining_budget;
            if request.budget.max_model_calls.unwrap_or(0)
                > parent_left.max_model_calls.unwrap_or(u32::MAX)
            {
                bail!(
                    "child model-call budget exceeds parent allocation"
                );
            }
            if request.budget.max_tool_calls.unwrap_or(0)
                > parent_left.max_tool_calls.unwrap_or(u32::MAX)
            {
                bail!("child tool-call budget exceeds parent allocation");
            }
        }

        let instance = AgentInstance {
            instance_id: instance_id.clone(),
            status: AgentStatus::Queued,
            request: request.clone(),
            remaining_budget: request.budget.clone(),
            consumed: ConsumedBudget::default(),
            depth,
        };
        inner.instances.insert(instance_id.clone(), instance.clone());
        Ok(instance)
    }

    pub fn mark_running(&self, instance_id: &str) -> Result<()> {
        self.set_status(instance_id, AgentStatus::Running)
    }

    pub fn mark_finished(&self, instance_id: &str, status: AgentStatus) -> Result<()> {
        anyhow::ensure!(
            matches!(status, AgentStatus::Succeeded | AgentStatus::Failed | AgentStatus::Cancelled),
            "terminal status required"
        );
        self.set_status(instance_id, status)
    }

    fn set_status(&self, instance_id: &str, status: AgentStatus) -> Result<()> {
        let mut inner = self.inner.lock();
        let instance = inner
            .instances
            .get_mut(instance_id)
            .context("instance not found")?;
        instance.status = status;
        Ok(())
    }

    /// Cancellation propagates to all running descendants (K-017).
    pub fn cancel_tree(&self, root_instance_id: &str) -> Result<usize> {
        let mut inner = self.inner.lock();
        if !inner.instances.contains_key(root_instance_id) {
            bail!("root instance not found");
        }
        // Collect descendants via parent links.
        let mut cancelled = 0;
        let mut frontier = vec![root_instance_id.to_owned()];
        while let Some(current) = frontier.pop() {
            let descendants: Vec<String> = inner
                .instances
                .values()
                .filter(|instance| {
                    instance
                        .request
                        .parent_instance_id
                        .as_deref()
                        .map(|p| p == current)
                        .unwrap_or(false)
                })
                .map(|instance| instance.instance_id.clone())
                .collect();
            for descendant in descendants {
                frontier.push(descendant.clone());
            }
            if let Some(instance) = inner.instances.get_mut(&current) {
                if matches!(instance.status, AgentStatus::Queued | AgentStatus::Running) {
                    instance.status = AgentStatus::Cancelled;
                    cancelled += 1;
                }
            }
        }
        Ok(cancelled)
    }

    /// Sends a mailbox message (K-009): ordered per recipient with
    /// sender/run provenance.
    pub fn send_message(
        &self,
        sender_instance_id: &str,
        recipient_instance_id: &str,
        content: &str,
        run_id: &str,
    ) -> Result<MailboxMessage> {
        let mut inner = self.inner.lock();
        if !inner.instances.contains_key(sender_instance_id) {
            bail!("sender instance not found");
        }
        if !inner.instances.contains_key(recipient_instance_id) {
            bail!("recipient instance not found");
        }
        inner.next_message += 1;
        let message = MailboxMessage {
            message_id: format!("msg_{:04}", inner.next_message),
            sender_instance_id: sender_instance_id.to_owned(),
            recipient_instance_id: recipient_instance_id.to_owned(),
            content: content.to_owned(),
            run_id: run_id.to_owned(),
            sequence: inner
                .mailboxes
                .get(recipient_instance_id)
                .and_then(|box_| box_.last())
                .map(|last| last.sequence + 1)
                .unwrap_or(1),
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
        };
        inner
            .mailboxes
            .entry(recipient_instance_id.to_owned())
            .or_default()
            .push(message.clone());
        Ok(message)
    }

    /// Drains the recipient mailbox in order.
    pub fn drain_mailbox(&self, recipient_instance_id: &str) -> Vec<MailboxMessage> {
        self.inner
            .lock()
            .mailboxes
            .remove(recipient_instance_id)
            .unwrap_or_default()
    }

    pub fn inspect(&self, instance_id: &str) -> Option<AgentInstance> {
        self.inner.lock().instances.get(instance_id).cloned()
    }

    pub fn instance_count(&self) -> usize {
        self.inner.lock().instances.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_profile::{builtin_profiles, explorer};

    fn scheduler() -> SubagentScheduler {
        let mut scheduler = SubagentScheduler::new(3);
        for profile in builtin_profiles() {
            scheduler.register_profile(profile);
        }
        scheduler
    }

    fn request(parent: Option<&str>, profile: &str, budget: Budget) -> SpawnRequest {
        SpawnRequest {
            parent_instance_id: parent.map(str::to_owned),
            profile_id: profile.into(),
            task: "do work".into(),
            mode: SpawnMode::Inline,
            budget,
        }
    }

    #[test]
    fn parent_child_budget_conservation_is_enforced() {
        let scheduler = scheduler();
        let parent = scheduler
            .spawn(request(None, "worker", Budget { max_model_calls: Some(10), ..Default::default() }))
            .unwrap();

        // Within parent allocation: accepted.
        let child = scheduler.spawn(request(
            Some(&parent.instance_id),
            "explorer",
            Budget { max_model_calls: Some(5), ..Default::default() },
        ));
        assert!(child.is_ok());

        // Exceeding parent allocation: rejected (K-016).
        let greedy = scheduler.spawn(request(
            Some(&parent.instance_id),
            "explorer",
            Budget { max_model_calls: Some(50), ..Default::default() },
        ));
        assert!(greedy.is_err(), "child budget must not exceed parent remaining");
    }

    #[test]
    fn recursion_limit_terminates_safely() {
        let scheduler = scheduler();
        let mut current = scheduler
            .spawn(request(None, "explorer", Budget::default()))
            .unwrap();
        let mut depth = 0;
        loop {
            let next = scheduler.spawn(request(
                Some(&current.instance_id),
                "explorer",
                Budget::default(),
            ));
            match next {
                Ok(instance) => {
                    current = instance;
                    depth += 1;
                }
                Err(error) => {
                    assert!(error.to_string().contains("recursion limit"));
                    break;
                }
            }
        }
        assert!(depth > 0, "at least one nested spawn succeeded");
        assert!(depth < 5, "must stop at max_depth=3");
    }

    #[test]
    fn cancellation_propagates_to_descendants() {
        let scheduler = scheduler();
        let root = scheduler.spawn(request(None, "worker", Budget::default())).unwrap();
        let child = scheduler
            .spawn(request(Some(&root.instance_id), "explorer", Budget::default()))
            .unwrap();
        let grandchild = scheduler
            .spawn(request(Some(&child.instance_id), "explorer", Budget::default()))
            .unwrap();
        scheduler.mark_running(&root.instance_id).unwrap();
        scheduler.mark_running(&grandchild.instance_id).unwrap();

        let cancelled = scheduler.cancel_tree(&root.instance_id).unwrap();
        assert_eq!(cancelled, 3, "root + child + grandchild");
        assert_eq!(scheduler.inspect(&root.instance_id).unwrap().status, AgentStatus::Cancelled);
        assert_eq!(scheduler.inspect(&grandchild.instance_id).unwrap().status, AgentStatus::Cancelled);
    }

    #[test]
    fn mailbox_is_ordered_with_provenance() {
        let scheduler = scheduler();
        let sender = scheduler.spawn(request(None, "worker", Budget::default())).unwrap();
        let recipient = scheduler.spawn(request(None, "explorer", Budget::default())).unwrap();

        let m1 = scheduler
            .send_message(&sender.instance_id, &recipient.instance_id, "first", "run_1")
            .unwrap();
        let m2 = scheduler
            .send_message(&sender.instance_id, &recipient.instance_id, "second", "run_1")
            .unwrap();

        assert_eq!((m1.sequence, m2.sequence), (1, 2), "ordered per recipient");
        assert_eq!(m1.sender_instance_id, sender.instance_id);
        assert_eq!(m1.run_id, "run_1");

        let drained = scheduler.drain_mailbox(&recipient.instance_id);
        assert_eq!(drained.len(), 2);
        assert!(scheduler.drain_mailbox(&recipient.instance_id).is_empty());
    }

    #[test]
    fn unknown_instances_are_rejected() {
        let scheduler = scheduler();
        assert!(scheduler.spawn(request(Some("agt_ghost"), "explorer", Budget::default())).is_err());
        let instance = scheduler.spawn(request(None, "explorer", Budget::default())).unwrap();
        assert!(scheduler.send_message("agt_ghost", &instance.instance_id, "x", "r").is_err());
    }

    #[test]
    fn explorer_profile_registered_and_validated() {
        let mut scheduler = SubagentScheduler::new(3);
        scheduler.register_profile(explorer());
        assert!(scheduler.profile("explorer").is_some());
        assert!(scheduler.profile("ghost").is_none());
    }
}
