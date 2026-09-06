//! Observational compaction (§19.4, Task 5.5, C-010, C-017).
//!
//! Converts long transcripts into chronological observations without
//! deleting raw messages. Observations preserve decisions, unresolved
//! questions, constraints, important tool outcomes, artifact references,
//! and user corrections — and link back to the source message ids.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceMessage {
    pub message_id: String,
    pub role: String,
    pub content: String,
    pub timestamp_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ObservationKind {
    Decision,
    UnresolvedQuestion,
    Constraint,
    ToolOutcome,
    UserCorrection,
    Progress,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    pub observation_id: String,
    pub kind: ObservationKind,
    pub summary: String,
    /// Source message ids the observation compresses (C-017 provenance).
    pub source_message_ids: Vec<String>,
    pub created_at_ms: i64,
}

/// Classifies a message into the observation taxonomy. Pure heuristics keep
/// the compactor deterministic — no model calls, no hidden state.
pub fn classify(message: &SourceMessage) -> Option<ObservationKind> {
    let content = message.content.to_lowercase();
    if message.role == "user"
        && (content.contains("yanlış")
            || content.contains("actually")
            || content.contains("bunun yerine")
            || content.contains("instead"))
    {
        return Some(ObservationKind::UserCorrection);
    }
    if message.role == "tool" {
        return Some(ObservationKind::ToolOutcome);
    }
    if content.contains("?")
        && (content.contains("nasıl") || content.contains("how") || content.contains("should"))
    {
        return Some(ObservationKind::UnresolvedQuestion);
    }
    if content.contains("karar")
        || content.contains("decided")
        || content.contains("we will use")
        || content.contains("seçtik")
    {
        return Some(ObservationKind::Decision);
    }
    if content.contains("must not")
        || content.contains("asla")
        || content.contains("constraint")
        || content.contains("kısıt")
    {
        return Some(ObservationKind::Constraint);
    }
    None
}

/// Compacts a transcript into observations. Consecutive tool messages of the
/// same tool merge into one observation; everything else one-to-one when
/// classifiable. Raw messages stay untouched in the session store.
pub fn compact(messages: &[SourceMessage]) -> Vec<Observation> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let mut observations = Vec::new();
    let mut counter = 0_u32;
    let mut tool_group: Vec<&SourceMessage> = Vec::new();

    let flush_tools = |group: &mut Vec<&SourceMessage>,
                       observations: &mut Vec<Observation>,
                       counter: &mut u32| {
        if group.is_empty() {
            return;
        }
        let summary = if group.len() == 1 {
            format!("tool outcome: {}", truncate(&group[0].content, 160))
        } else {
            format!(
                "{} tool outcomes, last: {}",
                group.len(),
                truncate(&group[group.len() - 1].content, 160)
            )
        };
        observations.push(Observation {
            observation_id: format!("obs_{:04}", *counter),
            kind: ObservationKind::ToolOutcome,
            summary,
            source_message_ids: group.iter().map(|m| m.message_id.clone()).collect(),
            created_at_ms: now,
        });
        *counter += 1;
        group.clear();
    };

    for message in messages {
        let kind = classify(message);
        if kind == Some(ObservationKind::ToolOutcome) {
            tool_group.push(message);
            continue;
        }
        flush_tools(&mut tool_group, &mut observations, &mut counter);
        if let Some(kind) = kind {
            observations.push(Observation {
                observation_id: format!("obs_{:04}", counter),
                kind,
                summary: truncate(&message.content, 200),
                source_message_ids: vec![message.message_id.clone()],
                created_at_ms: now,
            });
            counter += 1;
        }
    }
    flush_tools(&mut tool_group, &mut observations, &mut counter);
    observations
}

fn truncate(content: &str, max: usize) -> String {
    if content.len() <= max {
        content.to_owned()
    } else {
        let mut end = max;
        while !content.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &content[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(id: &str, role: &str, content: &str) -> SourceMessage {
        SourceMessage {
            message_id: id.into(),
            role: role.into(),
            content: content.into(),
            timestamp_ms: 1,
        }
    }

    #[test]
    fn classifies_decisions_and_corrections() {
        let decision = classify(&message(
            "m1",
            "assistant",
            "We decided to use SQLite WAL mode.",
        ));
        assert_eq!(decision, Some(ObservationKind::Decision));

        let correction = classify(&message("m2", "user", "actually bunun yerine ralph kullan"));
        assert_eq!(correction, Some(ObservationKind::UserCorrection));
    }

    #[test]
    fn unclassifiable_messages_are_dropped() {
        let observations = compact(&[message("m1", "assistant", "ok so let me check the file")]);
        assert!(
            observations.is_empty(),
            "plain chatter produces no observation"
        );
    }

    #[test]
    fn consecutive_tool_messages_merge_with_provenance() {
        let messages = vec![
            message("t1", "tool", "read: 10 lines"),
            message("t2", "tool", "read: 20 lines"),
            message(
                "t3",
                "assistant",
                "The constraint is: must not delete user files",
            ),
        ];
        let observations = compact(&messages);
        assert_eq!(observations.len(), 2);

        let merged = &observations[0];
        assert_eq!(merged.kind, ObservationKind::ToolOutcome);
        assert_eq!(
            merged.source_message_ids,
            vec!["t1".to_owned(), "t2".to_owned()]
        );
        assert!(merged.summary.contains("2 tool outcomes"));

        assert_eq!(observations[1].kind, ObservationKind::Constraint);
        assert_eq!(observations[1].source_message_ids, vec!["t3".to_owned()]);
    }

    #[test]
    fn long_content_is_truncated_on_char_boundary() {
        let long = "ş".repeat(500);
        let observations = compact(&[message(
            "m1",
            "assistant",
            &format!("We decided to accept {long}"),
        )]);
        assert_eq!(observations.len(), 1);
        assert!(observations[0].summary.chars().count() < 220);
    }

    #[test]
    fn every_observation_links_sources() {
        let messages = vec![
            message(
                "m1",
                "assistant",
                "We will use the kernel adapter for routing.",
            ),
            message("m2", "user", "how should timeouts work?"),
        ];
        for observation in compact(&messages) {
            assert!(!observation.source_message_ids.is_empty());
        }
    }
}
