//! Unified resource reader (§25.1, Task 9.2, D-001): text/code, JSON/YAML/
//! TOML, archive listings, SQLite metadata — read-only, never executes
//! document content.

use anyhow::{bail, Context as _, Result};
use serde_json::Value;
use std::path::Path;

pub struct ResourceReader;

impl ResourceReader {
    /// Reads a resource and returns a structured summary. Large/binary
    /// formats produce metadata, never arbitrary execution.
    pub fn read(path: &Path) -> Result<String> {
        if !path.exists() {
            bail!("resource not found: {}", path.display());
        }
        if path.is_dir() {
            return Self::read_directory(path);
        }
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_lowercase();
        match extension.as_str() {
            "json" => Self::read_json(path),
            "yaml" | "yml" | "toml" | "md" | "txt" | "rs" | "ts" | "tsx" | "js" | "jsx" | "py"
            | "go" | "java" | "c" | "h" | "cpp" | "cs" | "rb" | "css" | "html" | "proto"
            | "sql" | "lock" | "gitignore" => Self::read_text(path),
            "zip" => Self::read_zip(path),
            "sqlite" | "sqlite3" | "db" | "db3" => Self::read_sqlite_meta(path),
            "png" | "jpg" | "jpeg" | "gif" | "ico" | "pdf" | "exe" | "dll" | "so" | "wasm" => {
                Self::read_binary_meta(path)
            }
            _ => {
                // Sniff: if it decodes as UTF-8 text, read it; else metadata.
                let bytes = std::fs::read(path)?;
                if std::str::from_utf8(&bytes).is_ok() {
                    Self::read_text(path)
                } else {
                    Self::read_binary_meta(path)
                }
            }
        }
    }

    fn read_text(path: &Path) -> Result<String> {
        let content =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let lines = content.lines().count();
        Ok(format!("{} ({} lines)\n{}", path.display(), lines, content))
    }

    fn read_json(path: &Path) -> Result<String> {
        let content = std::fs::read_to_string(path)?;
        let value: Value = serde_json::from_str(&content)
            .with_context(|| format!("parsing {}", path.display()))?;
        let kind = match &value {
            Value::Object(map) => format!("object with {} keys", map.len()),
            Value::Array(items) => format!("array with {} items", items.len()),
            other => format!("scalar: {other}"),
        };
        Ok(format!("{} (json, {})\n{}", path.display(), kind, content))
    }

    fn read_directory(path: &Path) -> Result<String> {
        let mut entries: Vec<String> = Vec::new();
        for entry in
            std::fs::read_dir(path).with_context(|| format!("listing {}", path.display()))?
        {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let marker = if entry.path().is_dir() { "/" } else { "" };
            entries.push(format!("{name}{marker}"));
        }
        entries.sort();
        Ok(format!(
            "{} (directory, {} entries)\n{}",
            path.display(),
            entries.len(),
            entries.join("\n")
        ))
    }

    fn read_zip(path: &Path) -> Result<String> {
        let file = std::fs::File::open(path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        let mut listing = Vec::new();
        for index in 0..archive.len() {
            let entry = archive.by_index(index)?;
            listing.push(format!(
                "{} ({} bytes)",
                entry.name().to_owned(),
                entry.size()
            ));
        }
        listing.sort();
        Ok(format!(
            "{} (zip archive, {} entries)\n{}",
            path.display(),
            archive.len(),
            listing.join("\n")
        ))
    }

    /// SQLite metadata: table names + schema, read-only URI.
    fn read_sqlite_meta(path: &Path) -> Result<String> {
        // Deliberately minimal: file-size + magic header check without pulling
        // rusqlite into this crate; the daemon-side reader (with rusqlite)
        // handles full queries. Never executes anything from the file.
        let bytes = std::fs::read(path)?;
        if bytes.len() < 16 || &bytes[..16] != b"SQLite format 3\0" {
            bail!("{} is not a valid SQLite database", path.display());
        }
        Ok(format!(
            "{} (sqlite database, {} bytes, header valid; use the daemon tool for queries)",
            path.display(),
            bytes.len()
        ))
    }

    fn read_binary_meta(path: &Path) -> Result<String> {
        let metadata = std::fs::metadata(path)?;
        Ok(format!(
            "{} (binary, {} bytes; content not rendered)",
            path.display(),
            metadata.len()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_text_files_with_line_counts() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("notes.md");
        std::fs::write(&path, "one\ntwo\nthree").unwrap();
        let output = ResourceReader::read(&path).unwrap();
        assert!(output.contains("3 lines"));
        assert!(output.contains("three"));
    }

    #[test]
    fn reads_json_with_structure_summary() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("data.json");
        std::fs::write(&path, r#"{"a": 1, "b": [1, 2, 3]}"#).unwrap();
        let output = ResourceReader::read(&path).unwrap();
        assert!(output.contains("object with 2 keys"));
    }

    #[test]
    fn invalid_json_is_an_error_not_execution() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("bad.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert!(ResourceReader::read(&path).is_err());
    }

    #[test]
    fn directories_list_entries() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("a.txt"), "x").unwrap();
        std::fs::create_dir(temp.path().join("subdir")).unwrap();
        let output = ResourceReader::read(temp.path()).unwrap();
        assert!(output.contains("a.txt"));
        assert!(output.contains("subdir/"));
    }

    #[test]
    fn zip_lists_entries_without_extraction() {
        let temp = tempfile::tempdir().unwrap();
        let zip_path = temp.path().join("archive.zip");
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("inner.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut zip, b"hello").unwrap();
        zip.finish().unwrap();
        let output = ResourceReader::read(&zip_path).unwrap();
        assert!(output.contains("inner.txt"));
        assert!(!temp.path().join("inner.txt").exists(), "no extraction");
    }

    #[test]
    fn sqlite_magic_is_validated_without_execution() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("data.db");
        std::fs::write(&path, b"SQLite format 3\0garbage-but-not-executed").unwrap();
        let output = ResourceReader::read(&path).unwrap();
        assert!(output.contains("sqlite database"));
    }

    #[test]
    fn binaries_report_metadata_only() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("blob.bin");
        std::fs::write(&path, [0xFF, 0xD8, 0xFF, 0x00, 0x01]).unwrap();
        let output = ResourceReader::read(&path).unwrap();
        assert!(output.contains("binary"));
        assert!(output.contains("not rendered"));
    }

    #[test]
    fn missing_files_error() {
        assert!(ResourceReader::read(Path::new("Z:/missing.txt")).is_err());
    }
}
