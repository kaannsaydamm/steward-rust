//! Path-scoped rules (§20, Task 5.4, C-005): rule bodies load only when a
//! touched path matches the rule's globs.

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuleFrontmatter {
    pub id: String,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub priority: u32,
    #[serde(default)]
    pub always: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuleBody {
    pub frontmatter: RuleFrontmatter,
    pub body: String,
}

impl RuleBody {
    /// Parses a rule file: YAML frontmatter delimited by `---`, then body.
    pub fn parse(source: &str) -> Result<Self> {
        let trimmed = source.trim_start();
        let rest = trimmed
            .strip_prefix("---")
            .context("rule file must start with --- frontmatter")?;
        let (frontmatter_text, body) = rest
            .split_once("---")
            .context("rule frontmatter must close with ---")?;
        let frontmatter: RuleFrontmatter =
            serde_yaml::from_str(frontmatter_text).context("parsing rule frontmatter")?;
        Ok(Self {
            frontmatter,
            body: body.trim().to_owned(),
        })
    }

    /// Whether this rule applies to the given path. `always` rules match
    /// everything; otherwise any glob hit suffices. Empty globs match nothing
    /// unless `always`.
    pub fn matches(&self, path: &Path) -> bool {
        if self.frontmatter.always {
            return true;
        }
        let path_text = path.to_string_lossy().replace('\\', "/");
        self.frontmatter.paths.iter().any(|pattern| glob_match(pattern, &path_text))
    }
}

/// Minimal glob matcher supporting `*`, `**`, and `?`.
fn glob_match(pattern: &str, text: &str) -> bool {
    fn inner(p: &[u8], t: &[u8]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some(b'*'), _) => {
                if p.get(1) == Some(&b'*') {
                    // `**/` matches zero directories or any prefix.
                    if p.get(2) == Some(&b'/') {
                        inner(&p[3..], t) || (!t.is_empty() && inner(p, &t[1..]))
                    } else {
                        inner(&p[2..], t) || (!t.is_empty() && inner(p, &t[1..]))
                    }
                } else {
                    inner(&p[1..], t) || (!t.is_empty() && inner(p, &t[1..]))
                }
            }
            (Some(b'?'), Some(_)) => inner(&p[1..], &t[1..]),
            (Some(a), Some(b)) if a == b => inner(&p[1..], &t[1..]),
            _ => false,
        }
    }
    inner(pattern.as_bytes(), text.as_bytes())
}

#[derive(Clone, Debug, Default)]
pub struct RuleSet {
    pub rules: Vec<RuleBody>,
}

impl RuleSet {
    /// Loads all `*.md` rule files from a directory (non-recursive).
    pub fn load_directory(directory: &Path) -> Result<Self> {
        let mut rules = Vec::new();
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(_) => return Ok(Self { rules }),
        };
        for entry in entries {
            let entry = entry.context("reading rules dir entry")?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                let source = std::fs::read_to_string(&path)
                    .with_context(|| format!("reading rule {}", path.display()))?;
                let mut rule = RuleBody::parse(&source)
                    .with_context(|| format!("parsing rule {}", path.display()))?;
                // File-name tiebreak: include stem as fallback id.
                if rule.frontmatter.id.is_empty() {
                    rule.frontmatter.id = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or_default()
                        .to_owned();
                }
                rules.push(rule);
            }
        }
        rules.sort_by_key(|rule| std::cmp::Reverse(rule.frontmatter.priority));
        Ok(Self { rules })
    }

    /// Rules whose paths match, highest priority first (C-005).
    pub fn select_for_paths(&self, paths: &[&Path]) -> Vec<&RuleBody> {
        self.rules
            .iter()
            .filter(|rule| {
                rule.frontmatter.always || paths.iter().any(|path| rule.matches(path))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUST_RULE: &str = "---\nid: rust-defaults\npaths:\n  - \"crates/**/*.rs\"\npriority: 50\nalways: false\n---\nRun fmt and clippy before completion.";

    #[test]
    fn parses_frontmatter_and_body() {
        let rule = RuleBody::parse(RUST_RULE).unwrap();
        assert_eq!(rule.frontmatter.id, "rust-defaults");
        assert_eq!(rule.frontmatter.priority, 50);
        assert!(!rule.frontmatter.always);
        assert!(rule.body.contains("fmt and clippy"));
    }

    #[test]
    fn glob_matches_nested_paths() {
        let rule = RuleBody::parse(RUST_RULE).unwrap();
        assert!(rule.matches(Path::new("crates/kernel/src/lib.rs")));
        assert!(rule.matches(Path::new("crates/a/b/c.rs")));
        assert!(!rule.matches(Path::new("web-ui/src/app/page.tsx")));
        assert!(!rule.matches(Path::new("Cargo.toml")));
    }

    #[test]
    fn always_rules_match_anything() {
        let source = "---\nid: global\nalways: true\n---\nBe safe.";
        let rule = RuleBody::parse(source).unwrap();
        assert!(rule.matches(Path::new("anything/anywhere.txt")));
    }

    #[test]
    fn selector_loads_matching_only() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("rust.rule.md"),
            RUST_RULE,
        )
        .unwrap();
        std::fs::write(
            temp.path().join("global.rule.md"),
            "---\nid: global\nalways: true\n---\nAlways applies.",
        )
        .unwrap();

        let set = RuleSet::load_directory(temp.path()).unwrap();
        assert_eq!(set.rules.len(), 2);

        // Only the global rule matches this path.
        let selected = set.select_for_paths(&[Path::new("docs/readme.md")]);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].frontmatter.id, "global");

        // Both match a Rust path.
        let selected = set.select_for_paths(&[Path::new("crates/core/src/lib.rs")]);
        assert_eq!(selected.len(), 2);
        // Priority 50 rust rule sorts before the always rule (priority 0).
        assert_eq!(selected[0].frontmatter.id, "rust-defaults");
    }

    #[test]
    fn missing_directory_yields_empty_set() {
        let set = RuleSet::load_directory(Path::new("Z:/definitely/missing")).unwrap();
        assert!(set.rules.is_empty());
    }
}
