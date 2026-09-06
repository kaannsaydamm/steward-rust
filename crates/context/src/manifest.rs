//! Context manifests (§19.3, Task 5.3, C-001): every model call stores a
//! redacted manifest of what was included and why.

use crate::budget::PackedContext;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub id: String,
    pub tokens: u32,
    /// Why the item was included (`path_scope`, `dependency_rank`, ...).
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContextManifest {
    pub manifest_id: String,
    /// Model route that consumed this context (`auto/coding-heavy`...).
    pub model_route: String,
    pub budget_tokens: u32,
    pub packed_tokens: u32,
    pub selected: Vec<ManifestEntry>,
    pub omitted_count: u32,
    pub created_at_ms: i64,
}

impl ContextManifest {
    /// Builds a manifest from a packed context.
    pub fn from_packed(
        packed: &PackedContext,
        manifest_id: String,
        model_route: &str,
        budget_tokens: u32,
    ) -> Self {
        Self {
            manifest_id,
            model_route: model_route.to_owned(),
            budget_tokens,
            packed_tokens: packed.packed_tokens,
            selected: packed
                .selected
                .iter()
                .map(|item| ManifestEntry {
                    id: item.id.clone(),
                    tokens: item.estimated_tokens,
                    reason: format!("{:?}", item.source).to_snake_case(),
                })
                .collect(),
            omitted_count: packed.omitted.len() as u32,
            created_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }
}

trait ToSnakeCase {
    fn to_snake_case(&self) -> String;
}

impl ToSnakeCase for String {
    fn to_snake_case(&self) -> String {
        let mut out = String::with_capacity(self.len() + 4);
        for (index, ch) in self.chars().enumerate() {
            if ch.is_uppercase() {
                if index > 0 {
                    out.push('_');
                }
                out.extend(ch.to_lowercase());
            } else {
                out.push(ch);
            }
        }
        out
    }
}

impl ToSnakeCase for &str {
    fn to_snake_case(&self) -> String {
        let mut out = String::with_capacity(self.len() + 4);
        for (index, ch) in self.chars().enumerate() {
            if ch.is_uppercase() {
                if index > 0 {
                    out.push('_');
                }
                out.extend(ch.to_lowercase());
            } else {
                out.push(ch);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::{BudgetAllocator, BudgetPolicy};
    use crate::item::{ContextItem, ContextSourceKind, ContextScope, Sensitivity, TrustLevel};

    fn sample() -> PackedContext {
        let item = ContextItem {
            id: "rule:rust".into(),
            source: ContextSourceKind::ProjectRule,
            content: "run fmt".into(),
            estimated_tokens: 2,
            relevance: 0.9,
            authority: 1.0,
            freshness: 1.0,
            trust: TrustLevel::TrustedConfiguration,
            sensitivity: Sensitivity::Public,
            scope: ContextScope::Global,
        };
        BudgetAllocator::pack(vec![item], &BudgetPolicy::default()).unwrap()
    }

    #[test]
    fn manifest_roundtrips_through_json() {
        let packed = sample();
        let manifest = ContextManifest::from_packed(
            &packed,
            "mf_1".into(),
            "auto/coding",
            32_000,
        );
        let json = manifest.to_json();
        let back: ContextManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.manifest_id, "mf_1");
        assert_eq!(back.selected.len(), 1);
        assert_eq!(back.selected[0].id, "rule:rust");
        assert_eq!(back.model_route, "auto/coding");
    }

    #[test]
    fn every_inclusion_has_a_reason() {
        let packed = sample();
        let manifest =
            ContextManifest::from_packed(&packed, "mf_2".into(), "route", 100);
        for entry in &manifest.selected {
            assert!(!entry.reason.is_empty(), "entry {} lacks reason", entry.id);
        }
    }
}
