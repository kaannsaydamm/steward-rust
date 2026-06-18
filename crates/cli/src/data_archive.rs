use crate::cli::DataCommand;
use crate::client;
use anyhow::{bail, Context as _, Result};
use rusqlite::backup::Backup;
use rusqlite::Connection;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub async fn run(host: &str, command: DataCommand) -> Result<()> {
    let root = steward_core::storage::root().context("resolving ~/.steward data root")?;
    match command {
        DataCommand::Export(args) => {
            export(&root, &args.archive)?;
            println!("exported\t{}", args.archive.display());
        }
        DataCommand::Import(args) => {
            let daemon_is_running = matches!(
                tokio::time::timeout(Duration::from_millis(500), client::ping(host)).await,
                Ok(Ok(_))
            );
            if daemon_is_running {
                bail!("stop the Steward daemon before importing data");
            }
            import(&root, &args.archive)?;
            println!("imported\t{}", root.display());
        }
    }
    Ok(())
}

pub fn export(root: &Path, archive: &Path) -> Result<()> {
    if !root.is_dir() {
        bail!("Steward data root does not exist: {}", root.display());
    }
    if archive.starts_with(root) {
        bail!("export archive must be outside the Steward data root");
    }
    if let Some(parent) = archive.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let snapshot_dir = tempfile::tempdir()?;
    let snapshot_path = snapshot_dir.path().join("steward.db");
    let source_path = root.join("steward.db");
    if source_path.exists() {
        snapshot_database(&source_path, &snapshot_path)?;
    }

    let output = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(archive)?;
    let mut writer = ZipWriter::new(output);
    if snapshot_path.exists() {
        append_file(&mut writer, &snapshot_path, Path::new("steward.db"))?;
    }
    append_directory(&mut writer, root, root)?;
    writer.finish()?.sync_all()?;
    Ok(())
}

pub fn import(root: &Path, archive: &Path) -> Result<()> {
    let parent = root
        .parent()
        .context("Steward data root must have a parent directory")?;
    std::fs::create_dir_all(parent)?;
    let staging = tempfile::Builder::new()
        .prefix(".steward-import-")
        .tempdir_in(parent)?;
    extract_archive(archive, staging.path())?;
    validate_database(&staging.path().join("steward.db"))?;

    let backup = parent.join(format!(".steward-backup-{}", std::process::id()));
    if backup.exists() {
        bail!("stale import backup exists: {}", backup.display());
    }
    let had_existing_root = root.exists();
    if had_existing_root {
        std::fs::rename(root, &backup)?;
    }
    if let Err(error) = std::fs::rename(staging.path(), root) {
        if had_existing_root {
            let _ = std::fs::rename(&backup, root);
        }
        return Err(error).context("activating imported Steward data");
    }
    if had_existing_root {
        std::fs::remove_dir_all(backup)?;
    }
    Ok(())
}

fn snapshot_database(source_path: &Path, snapshot_path: &Path) -> Result<()> {
    let source = Connection::open(source_path)?;
    let mut destination = Connection::open(snapshot_path)?;
    let backup = Backup::new(&source, &mut destination)?;
    backup.run_to_completion(128, Duration::from_millis(10), None)?;
    Ok(())
}

fn append_directory(writer: &mut ZipWriter<File>, root: &Path, directory: &Path) -> Result<()> {
    let mut entries = std::fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            append_directory(writer, root, &path)?;
            continue;
        }
        let relative = path.strip_prefix(root)?;
        if matches!(
            relative.to_string_lossy().as_ref(),
            "steward.db" | "steward.db-wal" | "steward.db-shm"
        ) {
            continue;
        }
        append_file(writer, &path, relative)?;
    }
    Ok(())
}

fn append_file(writer: &mut ZipWriter<File>, source: &Path, relative: &Path) -> Result<()> {
    let archive_name = portable_path(relative)?;
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer.start_file(archive_name, options)?;
    let mut input = File::open(source)?;
    std::io::copy(&mut input, writer)?;
    Ok(())
}

fn portable_path(path: &Path) -> Result<String> {
    let parts = path
        .components()
        .map(|component| component.as_os_str().to_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()
        .context("Steward data path is not valid UTF-8")?;
    Ok(parts.join("/"))
}

fn extract_archive(archive: &Path, destination: &Path) -> Result<()> {
    let mut archive = ZipArchive::new(File::open(archive)?)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let relative = entry
            .enclosed_name()
            .context("archive contains an unsafe path")?
            .to_path_buf();
        reject_symlink(&entry)?;
        let output = destination.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(output)?;
            continue;
        }
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = File::create(output)?;
        std::io::copy(&mut entry, &mut file)?;
        file.flush()?;
    }
    Ok(())
}

fn reject_symlink(entry: &zip::read::ZipFile<'_>) -> Result<()> {
    if entry
        .unix_mode()
        .is_some_and(|mode| mode & 0o170_000 == 0o120_000)
    {
        bail!("archive contains a symbolic link");
    }
    Ok(())
}

fn validate_database(path: &Path) -> Result<()> {
    if !path.is_file() {
        bail!("archive does not contain steward.db");
    }
    let connection = Connection::open(path)?;
    let status: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if status != "ok" {
        bail!("imported database failed SQLite quick_check: {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{export, import};
    use rusqlite::Connection;
    use std::io::Write as _;
    use zip::write::SimpleFileOptions;

    #[test]
    fn archive_round_trip_restores_database_and_nested_files() {
        let temp = tempfile::tempdir().expect("create temp directory");
        let root = temp.path().join(".steward");
        std::fs::create_dir_all(root.join("memory/nightly")).expect("create data root");
        let database = Connection::open(root.join("steward.db")).expect("open database");
        database
            .execute_batch(
                "CREATE TABLE session (value TEXT); INSERT INTO session VALUES ('kept');",
            )
            .expect("seed database");
        drop(database);
        std::fs::write(root.join("memory/nightly/2026-06-18.md"), "portable memory")
            .expect("write memory");
        let archive = temp.path().join("backup.steward.zip");

        export(&root, &archive).expect("export data");
        std::fs::remove_dir_all(&root).expect("remove source data");
        import(&root, &archive).expect("import data");

        let restored = Connection::open(root.join("steward.db")).expect("open restored database");
        let value: String = restored
            .query_row("SELECT value FROM session", [], |row| row.get(0))
            .expect("read restored session");
        assert_eq!(value, "kept");
        assert_eq!(
            std::fs::read_to_string(root.join("memory/nightly/2026-06-18.md"))
                .expect("read restored memory"),
            "portable memory"
        );
    }

    #[test]
    fn import_rejects_unsafe_paths_without_replacing_existing_data() {
        let temp = tempfile::tempdir().expect("create temp directory");
        let root = temp.path().join(".steward");
        std::fs::create_dir_all(&root).expect("create existing root");
        std::fs::write(root.join("marker"), "unchanged").expect("write marker");
        let archive_path = temp.path().join("unsafe.zip");
        let file = std::fs::File::create(&archive_path).expect("create archive");
        let mut archive = zip::ZipWriter::new(file);
        archive
            .start_file("../outside", SimpleFileOptions::default())
            .expect("start unsafe entry");
        archive.write_all(b"escape").expect("write unsafe entry");
        archive.finish().expect("finish archive");

        let error = import(&root, &archive_path).expect_err("reject unsafe archive");

        assert!(error.to_string().contains("unsafe path"));
        assert_eq!(
            std::fs::read_to_string(root.join("marker")).expect("read marker"),
            "unchanged"
        );
    }

    #[test]
    fn import_rejects_corrupt_database_without_replacing_existing_data() {
        let temp = tempfile::tempdir().expect("create temp directory");
        let root = temp.path().join(".steward");
        std::fs::create_dir_all(&root).expect("create existing root");
        std::fs::write(root.join("marker"), "unchanged").expect("write marker");
        let archive_path = temp.path().join("corrupt.zip");
        let file = std::fs::File::create(&archive_path).expect("create archive");
        let mut archive = zip::ZipWriter::new(file);
        archive
            .start_file("steward.db", SimpleFileOptions::default())
            .expect("start database entry");
        archive
            .write_all(b"not sqlite")
            .expect("write corrupt data");
        archive.finish().expect("finish archive");

        let error = import(&root, &archive_path).expect_err("reject corrupt database");

        assert!(error.to_string().contains("file is not a database"));
    }
}
