//! Model capability catalog (§21.1, M-004, M-006).

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Route identifier: `provider/model` (opaque string at the wire boundary).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RouteId(pub String);

impl RouteId {
    pub fn new(provider: &str, model: &str) -> Self {
        Self(format!("{provider}/{model}"))
    }
}

/// Normalized capability metadata (M-006): explicit, never inferred by name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelCapabilities {
    pub max_context_tokens: u64,
    pub max_output_tokens: u64,
    pub tool_use: bool,
    pub parallel_tool_use: bool,
    pub structured_output: bool,
    pub vision: bool,
    pub audio: bool,
    pub reasoning_controls: bool,
    pub prompt_caching: bool,
    pub native_web_search: bool,
    /// Local-only routes can satisfy privacy requirements.
    pub local_only: bool,
    /// Soft score inputs.
    pub cost_class: u8,      // 0 cheapest..255
    pub latency_class: u8,   // 0 fastest..255
}

impl Default for ModelCapabilities {
    fn default() -> Self {
        Self {
            max_context_tokens: 128_000,
            max_output_tokens: 8_192,
            tool_use: false,
            parallel_tool_use: false,
            structured_output: false,
            vision: false,
            audio: false,
            reasoning_controls: false,
            prompt_caching: false,
            native_web_search: false,
            local_only: false,
            cost_class: 128,
            latency_class: 128,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelEntry {
    pub route: RouteId,
    pub capabilities: ModelCapabilities,
    /// User-declared metadata (display name, tags).
    #[serde(default)]
    pub tags: Vec<String>,
}

/// The catalog: known routes plus user-registered custom models.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Catalog {
    entries: BTreeMap<String, ModelEntry>,
}

impl Catalog {
    pub fn register(&mut self, entry: ModelEntry) {
        self.entries.insert(entry.route.0.clone(), entry);
    }

    pub fn get(&self, route: &str) -> Option<&ModelEntry> {
        self.entries.get(route)
    }

    pub fn all(&self) -> impl Iterator<Item = &ModelEntry> {
        self.entries.values()
    }

    /// Loads a user catalog file; missing file yields an empty catalog.
    pub fn load(path: &std::path::Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(self)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(route: &str, tool_use: bool, context: u64) -> ModelEntry {
        ModelEntry {
            route: RouteId(route.into()),
            capabilities: ModelCapabilities {
                tool_use,
                max_context_tokens: context,
                ..Default::default()
            },
            tags: vec![],
        }
    }

    #[test]
    fn registers_and_looks_up_routes() {
        let mut catalog = Catalog::default();
        catalog.register(entry("openai/gpt-x", true, 200_000));
        catalog.register(entry("ollama/local", false, 32_000));

        assert!(catalog.get("openai/gpt-x").unwrap().capabilities.tool_use);
        assert_eq!(catalog.get("ollama/local").unwrap().capabilities.max_context_tokens, 32_000);
        assert!(catalog.get("missing/model").is_none());
    }

    #[test]
    fn custom_models_need_no_code_registration() {
        // M-004: user declares capabilities for an unknown endpoint.
        let mut catalog = Catalog::default();
        catalog.register(ModelEntry {
            route: RouteId::new("bankofai", "glm-5.3-flash"),
            capabilities: ModelCapabilities {
                tool_use: true,
                vision: false,
                ..Default::default()
            },
            tags: vec!["fast".into()],
        });
        assert!(catalog.get("bankofai/glm-5.3-flash").is_some());
    }

    #[test]
    fn catalog_saves_and_loads_roundtrip() {
        let mut catalog = Catalog::default();
        catalog.register(entry("prov/model", true, 128_000));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model-catalog.json");
        catalog.save(&path).unwrap();
        let loaded = Catalog::load(&path).unwrap();
        assert_eq!(loaded.get("prov/model"), catalog.get("prov/model"));
    }

    #[test]
    fn missing_catalog_file_is_empty_not_error() {
        let loaded = Catalog::load(std::path::Path::new("Z:/none/catalog.json")).unwrap();
        assert_eq!(loaded.all().count(), 0);
    }
}
