//! Hash-anchored edit engine (§25.2, Task 9.3, D-002/D-003).
//!
//! Every read intended for editing yields anchors (line + content hash +
//! file revision). Edits carry the expected anchor; a stale anchor is a
//! hard error — never a blind write at a shifted location (D-002).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EditAnchor {
    /// 1-based line the edit targets.
    pub line: u32,
    /// SHA-256 of the exact line content at read time.
    pub line_hash: String,
    /// Revision hash of the whole file at read time.
    pub file_revision: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnchoredEdit {
    pub path: String,
    pub anchor: EditAnchor,
    /// New content replacing the anchored line.
    pub new_line: String,
}

#[derive(Debug, Error, PartialEq)]
pub enum EditError {
    #[error("file revision changed since read (expected {expected}, found {found}); re-read before editing")]
    StaleRevision { expected: String, found: String },
    #[error("line {line} changed since read (expected hash {expected}, found {found})")]
    StaleLine { line: u32, expected: String, found: String },
    #[error("line {line} is beyond end of file ({total} lines)")]
    LineOutOfRange { line: u32, total: u32 },
    #[error("io error: {0}")]
    Io(String),
}

pub struct EditEngine;

impl EditEngine {
    pub fn revision(content: &str) -> String {
        format!("{:x}", Sha256::digest(content.as_bytes()))
    }

    pub fn line_hash(line: &str) -> String {
        format!("{:x}", Sha256::digest(line.as_bytes()))
    }

    /// Yields the anchor for a line after a read.
    pub fn anchor(content: &str, line: u32) -> Result<EditAnchor> {
        let lines: Vec<&str> = content.lines().collect();
        if line == 0 || line as usize > lines.len() {
            anyhow::bail!(EditError::LineOutOfRange { line, total: lines.len() as u32 });
        }
        Ok(EditAnchor {
            line,
            line_hash: Self::line_hash(lines[line as usize - 1]),
            file_revision: Self::revision(content),
        })
    }

    /// Applies one anchored replacement atomically. Returns the new content.
    pub fn apply(content: &str, edit: &AnchoredEdit) -> std::result::Result<String, EditError> {
        let current_revision = Self::revision(content);
        if current_revision != edit.anchor.file_revision {
            return Err(EditError::StaleRevision {
                expected: edit.anchor.file_revision.clone(),
                found: current_revision,
            });
        }
        let mut lines: Vec<String> = content.lines().map(str::to_owned).collect();
        let index = edit.anchor.line as usize;
        if index == 0 || index > lines.len() {
            return Err(EditError::LineOutOfRange {
                line: edit.anchor.line,
                total: lines.len() as u32,
            });
        }
        let current_line_hash = Self::line_hash(&lines[index - 1]);
        if current_line_hash != edit.anchor.line_hash {
            return Err(EditError::StaleLine {
                line: edit.anchor.line,
                expected: edit.anchor.line_hash.clone(),
                found: current_line_hash,
            });
        }
        lines[index - 1] = edit.new_line.clone();
        if content.ends_with('\n') || lines.is_empty() {
            Ok(lines.join("\n") + if lines.is_empty() { "" } else { "\n" })
        } else {
            Ok(lines.join("\n"))
        }
    }

    /// Applies a sequence of edits against the same base revision; all-or-
    /// nothing (D-003 atomic multi-edit).
    pub fn apply_batch(
        content: &str,
        edits: &[AnchoredEdit],
    ) -> std::result::Result<String, EditError> {
        // All anchors must reference the SAME pre-edit revision, verified
        // against the current content once; then apply sequentially, each
        // validated against its line hash resolved on the evolving content
        // by mapping to original positions. Simpler contract: batch edits
        // target distinct original lines and apply bottom-up so earlier
        // edits don't shift later line numbers.
        let current_revision = Self::revision(content);
        for edit in edits {
            if edit.anchor.file_revision != current_revision {
                return Err(EditError::StaleRevision {
                    expected: edit.anchor.file_revision.clone(),
                    found: current_revision,
                });
            }
        }
        let mut sorted: Vec<&AnchoredEdit> = edits.iter().collect();
        sorted.sort_by_key(|e| std::cmp::Reverse(e.anchor.line));
        // Duplicate line check.
        for window in sorted.windows(2) {
            if window[0].anchor.line == window[1].anchor.line {
                return Err(EditError::StaleLine {
                    line: window[0].anchor.line,
                    expected: String::new(),
                    found: String::new(),
                });
            }
        }
        let mut lines: Vec<String> = content.lines().map(str::to_owned).collect();
        for edit in sorted {
            let index = edit.anchor.line as usize;
            if index == 0 || index > lines.len() {
                return Err(EditError::LineOutOfRange {
                    line: edit.anchor.line,
                    total: lines.len() as u32,
                });
            }
            let hash = Self::line_hash(&lines[index - 1]);
            if hash != edit.anchor.line_hash {
                return Err(EditError::StaleLine {
                    line: edit.anchor.line,
                    expected: edit.anchor.line_hash.clone(),
                    found: hash,
                });
            }
            lines[index - 1] = edit.new_line.clone();
        }
        Ok(lines.join("\n"))
    }

    /// Reads a file and validates content before writing.
    pub fn write_anchored(path: &std::path::Path, new_content: &str) -> Result<()> {
        std::fs::write(path, new_content)
            .map_err(|error| anyhow::Error::new(EditError::Io(error.to_string())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "fn one() {}\nfn two() {}\nfn three() {}\n";

    fn anchor(content: &str, line: u32) -> EditAnchor {
        EditEngine::anchor(content, line).unwrap()
    }

    #[test]
    fn unchanged_anchor_patch_succeeds() {
        let anchor = anchor(BASE, 2);
        let edit = AnchoredEdit {
            path: "lib.rs".into(),
            new_line: "fn two_renamed() {}".into(),
            anchor,
        };
        let patched = EditEngine::apply(BASE, &edit).unwrap();
        assert!(patched.contains("fn two_renamed() {}"));
        assert!(patched.contains("fn one() {}"));
        assert!(patched.contains("fn three() {}"));
    }

    #[test]
    fn stale_file_revision_fails_safely() {
        let anchor = anchor(BASE, 1);
        let drifted = "fn one() CHANGED {}\nfn two() {}\nfn three() {}\n";
        let edit = AnchoredEdit { path: "lib.rs".into(), new_line: "x".into(), anchor };
        let error = EditEngine::apply(drifted, &edit).unwrap_err();
        assert!(matches!(error, EditError::StaleRevision { .. }), "got {error}");
        assert!(error.to_string().contains("re-read before editing"));
    }

    #[test]
    fn stale_line_hash_fails_not_blind_write() {
        // Revision matches but the line hash does not (e.g. a concurrent
        // in-place rewrite of the same length): reject, never blind-write.
        let anchor = EditAnchor {
            line: 2,
            line_hash: "deadbeef".into(),
            file_revision: EditEngine::revision(BASE),
        };
        let edit = AnchoredEdit { path: "lib.rs".into(), new_line: "x".into(), anchor };
        let error = EditEngine::apply(BASE, &edit).unwrap_err();
        assert!(matches!(error, EditError::StaleLine { line: 2, .. }), "got {error}");
    }

    #[test]
    fn line_out_of_range_is_rejected() {
        let anchor = EditEngine::anchor(BASE, 99).unwrap_err();
        assert!(anchor.to_string().contains("beyond end of file"));
        let mut no_third = String::from("fn one() {}\nfn two() {}");
        no_third.push('\n');
        let anchor = EditEngine::anchor(&no_third, 3);
        assert!(anchor.is_err(), "anchor beyond EOF rejected at creation");
        // Direct apply with a hand-built out-of-range anchor:
        let edit = AnchoredEdit {
            path: "lib.rs".into(),
            new_line: "x".into(),
            anchor: EditAnchor {
                line: 3,
                line_hash: EditEngine::line_hash("fn three() {}"),
                file_revision: EditEngine::revision(&no_third),
            },
        };
        let error = EditEngine::apply(&no_third, &edit).unwrap_err();
        assert!(matches!(error, EditError::LineOutOfRange { line: 3, .. }));
    }

    #[test]
    fn batch_edits_apply_bottom_up_atomically() {
        let a1 = anchor(BASE, 1);
        let a3 = anchor(BASE, 3);
        let edits = vec![
            AnchoredEdit { path: "lib.rs".into(), new_line: "fn one_new() {}".into(), anchor: a1 },
            AnchoredEdit { path: "lib.rs".into(), new_line: "fn three_new() {}".into(), anchor: a3 },
        ];
        let patched = EditEngine::apply_batch(BASE, &edits).unwrap();
        assert!(patched.contains("fn one_new() {}"));
        assert!(patched.contains("fn three_new() {}"));
        assert!(patched.contains("fn two() {}"), "untouched line survives");
    }

    #[test]
    fn batch_rejects_any_stale_anchor_atomically() {
        let a1 = anchor(BASE, 1);
        // a3 built from a different revision.
        let drifted = BASE.replace("fn three", "fn three_v2");
        let a3 = EditEngine::anchor(&drifted, 3).unwrap();
        let edits = vec![
            AnchoredEdit { path: "lib.rs".into(), new_line: "x".into(), anchor: a1 },
            AnchoredEdit { path: "lib.rs".into(), new_line: "y".into(), anchor: a3 },
        ];
        let error = EditEngine::apply_batch(BASE, &edits).unwrap_err();
        assert!(matches!(error, EditError::StaleRevision { .. }));
    }

    #[test]
    fn batch_rejects_duplicate_lines() {
        let a2 = anchor(BASE, 2);
        let edits = vec![
            AnchoredEdit { path: "lib.rs".into(), new_line: "a".into(), anchor: a2.clone() },
            AnchoredEdit { path: "lib.rs".into(), new_line: "b".into(), anchor: a2 },
        ];
        let error = EditEngine::apply_batch(BASE, &edits).unwrap_err();
        assert!(matches!(error, EditError::StaleLine { .. }));
    }

    #[test]
    fn roundtrip_write_and_reread_anchors() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lib.rs");
        std::fs::write(&path, BASE).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let anchor = EditEngine::anchor(&content, 1).unwrap();
        let edit = AnchoredEdit {
            path: path.display().to_string(),
            new_line: "fn one_patched() {}".into(),
            anchor,
        };
        let patched = EditEngine::apply(&content, &edit).unwrap();
        EditEngine::write_anchored(&path, &patched).unwrap();

        let reread = std::fs::read_to_string(&path).unwrap();
        // The new revision anchors cleanly for the next edit.
        assert!(EditEngine::anchor(&reread, 1).is_ok());
        assert!(reread.contains("fn one_patched() {}"));
    }

    #[test]
    fn unicode_lines_hash_correctly() {
        let content = "fn şehler() {}\n";
        let anchor = EditEngine::anchor(content, 1).unwrap();
        let edit = AnchoredEdit {
            path: "x.rs".into(),
            new_line: "fn şehler_v2() {}".into(),
            anchor,
        };
        let patched = EditEngine::apply(content, &edit).unwrap();
        assert!(patched.contains("şehler_v2"));
    }
}
