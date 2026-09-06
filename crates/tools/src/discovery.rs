//! Lazy tool discovery (§23.4, Task 7.5, C-007): the kernel starts with a
//! compact core set; other tools surface through `tool.search`.

use crate::spec::ToolSpec;
use parking_lot::Mutex;
use std::collections::BTreeMap;

/// BM25-lite: substring + tag matches over the description. Good enough for
/// deterministic tests; SQLite FTS rides in the daemon integration.
pub struct ToolIndex {
    tools: Mutex<BTreeMap<String, ToolSpec>>,
    /// Always-visible core tool ids per profile.
    core: Vec<String>,
}

impl ToolIndex {
    pub fn new(core: Vec<String>) -> Self {
        Self { tools: Mutex::new(BTreeMap::new()), core }
    }

    pub fn insert(&self, spec: ToolSpec) {
        self.tools.lock().insert(spec.id.clone(), spec);
    }

    pub fn len(&self) -> usize {
        self.tools.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The always-visible core set (Task 7.5: fewer than configured visible
    /// schemas even with hundreds registered).
    pub fn core_tools(&self) -> Vec<ToolSpec> {
        let tools = self.tools.lock();
        self.core
            .iter()
            .filter_map(|id| tools.get(id).cloned())
            .collect()
    }

    /// Deterministic search: score = name hit (3) + tag hit (2) + description
    /// substring hits, sorted by score desc then id.
    pub fn search(&self, query: &str, limit: usize) -> Vec<ToolSpec> {
        let query = query.to_lowercase();
        let tools = self.tools.lock();
        let mut scored: Vec<(i64, &ToolSpec)> = tools
            .values()
            .filter_map(|spec| {
                let name_hits = (substring_count(&spec.id.to_lowercase(), &query) * 3) as i64;
                let tag_hits: i64 = spec
                    .tags
                    .iter()
                    .map(|t| (substring_count(&t.to_lowercase(), &query) * 2) as i64)
                    .sum();
                let desc_hits = substring_count(&spec.description.to_lowercase(), &query) as i64;
                let score = name_hits + tag_hits + desc_hits;
                (score > 0).then_some((score, spec))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
        scored
            .into_iter()
            .take(limit)
            .map(|(_, spec)| spec.clone())
            .collect()
    }
}

fn substring_count(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::Effect;
    use crate::spec::{RiskLevel, ToolSpec};
    use serde_json::json;

    fn tool(id: &str, description: &str, tags: &[&str]) -> ToolSpec {
        ToolSpec {
            id: id.into(),
            title: id.into(),
            description: description.into(),
            schema: json!({}),
            effects: vec![Effect::FilesystemRead],
            risk: RiskLevel::Low,
            tags: tags.iter().map(|t| t.to_string()).collect(),
            provider: "builtin".into(),
        }
    }

    fn index_with_200_tools() -> ToolIndex {
        let index = ToolIndex::new(vec!["fs.read".into(), "fs.search".into(), "fs.edit".into()]);
        for i in 0..197 {
            index.insert(tool(&format!("misc.tool{i}"), &format!("generic helper number {i}"), &[]));
        }
        index.insert(tool(
            "k8s.logs",
            "fetch kubernetes pod logs",
            &["kubernetes", "k8s", "ops"],
        ));
        index.insert(tool(
            "k8s.scale",
            "scale a kubernetes deployment",
            &["kubernetes", "k8s", "ops"],
        ));
        index.insert(tool("fs.read", "read a workspace file", &["fs"]));
        index.insert(tool("fs.search", "search workspace files", &["fs"]));
        index.insert(tool("fs.edit", "edit a workspace file", &["fs", "write"]));
        index
    }

    #[test]
    fn core_set_stays_small_with_200_registered() {
        let index = index_with_200_tools();
        assert_eq!(index.len(), 202);
        let core = index.core_tools();
        assert_eq!(core.len(), 3, "only the core set is visible by default");
        assert!(core.iter().all(|t| t.id.starts_with("fs.")));
    }

    #[test]
    fn search_recovers_tagged_tools() {
        let index = index_with_200_tools();
        let hits = index.search("kubernetes", 10);
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|t| t.id.starts_with("k8s.")));
    }

    #[test]
    fn search_ranks_name_hits_above_description() {
        let index = ToolIndex::new(vec![]);
        index.insert(tool("fs.read", "read files", &[]));
        index.insert(tool("other", "you can read things here too", &[]));
        let hits = index.search("read", 5);
        assert_eq!(hits[0].id, "fs.read", "name hit ranks first");
    }

    #[test]
    fn no_match_returns_empty() {
        let index = index_with_200_tools();
        assert!(index.search("zzz-nothing", 5).is_empty());
    }
}
