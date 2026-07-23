//! Local rotating backup, restore, and migration safety for the durable
//! SQLite database.
//!
//! Backups live in the macOS application-data area under a `backups`
//! subdirectory. Every backup is a single, transactionally-consistent SQLite
//! file produced by `VACUUM INTO` — never a naive copy of a live WAL
//! database — paired with a JSON manifest sidecar that records the schema
//! version, the moment, the app version, a sha256 checksum, and the backup
//! kind.
//!
//! This module owns the SQLite-safe semantics, the manifest format, the
//! checksum, integrity validation, retention, and atomic file replacement.
//! It is connection-agnostic: callers pass a `&Connection` (for creating a
//! backup) or a path (for everything else). The durable `WorkspaceStore` and
//! the Tauri command layer own when a backup happens and how the live
//! connection is closed and reopened around a restore.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

/// The newest schema version this build of Nodepad understands. A backup
/// whose manifest names a higher version is rejected before restore so an
/// older app never silently downgrades data it cannot migrate.
pub const SUPPORTED_SCHEMA_VERSION: i64 = 9;

/// Keep the latest seven valid automatic backups. The cap is enforced on
/// every automatic backup; older automatic backups beyond seven are deleted.
pub const MAX_AUTOMATIC_BACKUPS: usize = 7;
/// Pre-migration backups are kept separately from automatic backups so a
/// migration-heavy day cannot crowd out routine coverage. Four is enough to
/// step back through a chain of migrations.
pub const MAX_PRE_MIGRATION_BACKUPS: usize = 4;
/// Pre-restore backups are capped for the same reason as pre-migration.
pub const MAX_PRE_RESTORE_BACKUPS: usize = 4;

const SQLITE_SUFFIX: &str = "sqlite";
const MANIFEST_SUFFIX: &str = "manifest.json";
const STATE_FILE: &str = "backup-state.json";

/// Bookkeeping for the automatic-backup check, kept beside the backups rather
/// than in the thinker's database so writing it never changes the durable-data
/// fingerprint it is meant to compare against. Without this separation the
/// fingerprint would always read "changed" the moment a backup recorded its
/// own tracking row.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackupState {
    pub last_automatic_day: Option<String>,
    pub last_fingerprint: Option<String>,
}

impl BackupState {
    fn path(dir: &Path) -> PathBuf {
        dir.join(STATE_FILE)
    }
}

/// Reads the automatic-backup bookkeeping, or a default (no prior backup) when
/// the file is absent or unreadable. A corrupt state file is treated as "no
/// state" so a thinker is never blocked from a fresh backup by a bad sidecar.
pub fn read_backup_state(dir: &Path) -> BackupState {
    let Ok(bytes) = fs::read(BackupState::path(dir)) else {
        return BackupState::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

/// Writes the automatic-backup bookkeeping atomically.
pub fn write_backup_state(dir: &Path, state: &BackupState) -> Result<(), BackupError> {
    ensure_backups_dir(dir)?;
    let bytes = serde_json::to_vec_pretty(state).map_err(|_| BackupError::Manifest)?;
    let final_path = BackupState::path(dir);
    let tmp = final_path.with_extension("json.tmp");
    fs::write(&tmp, &bytes).map_err(|_| BackupError::Manifest)?;
    fs::rename(&tmp, &final_path).map_err(|_| BackupError::Manifest)?;
    Ok(())
}

/// Why a backup could not be created, validated, or restored. Each variant is
/// a typed recovery state the UI can render without inspecting the message.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum BackupError {
    #[error("Nodepad could not reach its local backups folder: {0}")]
    BackupsDir(String),
    #[error("Nodepad could not create the backup: {0}")]
    Create(String),
    #[error("That backup no longer exists.")]
    Missing,
    #[error("The backup file is unreadable: {0}")]
    Read(String),
    #[error("The backup checksum does not match its manifest.")]
    ChecksumMismatch,
    #[error("The backup database failed its integrity check.")]
    Corrupt,
    #[error("The backup schema is newer than this Nodepad supports.")]
    UnsupportedSchema,
    #[error("The backup manifest is unreadable.")]
    Manifest,
    #[error("Nodepad could not replace the local database: {0}")]
    Replace(String),
    #[error("Nodepad could not reopen its local database: {0}")]
    Reopen(String),
}

/// What kind of backup a file is. Drives retention: automatic backups are
/// capped at [`MAX_AUTOMATIC_BACKUPS`]; the two explicit kinds are capped at
/// their own smaller limits and never compete with automatic slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupKind {
    Automatic,
    PreMigration,
    PreRestore,
}

impl BackupKind {
    fn retention_limit(self) -> usize {
        match self {
            BackupKind::Automatic => MAX_AUTOMATIC_BACKUPS,
            BackupKind::PreMigration => MAX_PRE_MIGRATION_BACKUPS,
            BackupKind::PreRestore => MAX_PRE_RESTORE_BACKUPS,
        }
    }
}

/// The metadata stored beside a backup file. The manifest is the only thing
/// the recovery screen reads to list backups; the checksum and schema version
/// are re-verified before any restore replaces current data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub id: String,
    pub kind: BackupKind,
    pub schema_version: i64,
    pub created_at: String,
    pub app_version: String,
    pub checksum: String,
}

/// Where backups live: a `backups` subdirectory of the macOS application-data
/// area. The caller is responsible for creating it via [`ensure_backups_dir`].
pub fn backups_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("backups")
}

pub fn ensure_backups_dir(dir: &Path) -> Result<(), BackupError> {
    fs::create_dir_all(dir).map_err(|error| {
        BackupError::BackupsDir(format!(
            "Nodepad could not create its local backups folder: {error}"
        ))
    })
}

fn backup_db_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.{SQLITE_SUFFIX}"))
}

fn manifest_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.{MANIFEST_SUFFIX}"))
}

/// Creates one transactionally-consistent backup of the open database using
/// `VACUUM INTO`, then writes the manifest sidecar. The backup is verified
/// before this function returns: a backup whose own integrity check fails is
/// deleted and reported as an error, so a caller that runs this before a
/// migration never proceeds on top of an invalid backup.
///
/// `now` is supplied (RFC3339) rather than read here so tests drive the clock
/// deterministically. The database must already be committed to the state the
/// caller wants preserved.
pub fn create_backup(
    connection: &Connection,
    dir: &Path,
    kind: BackupKind,
    now: &str,
    app_version: &str,
) -> Result<BackupManifest, BackupError> {
    ensure_backups_dir(dir)?;
    let id = Uuid::new_v4().to_string();
    let target = backup_db_path(dir, &id);
    // `VACUUM INTO` does not accept a bound parameter for the filename in the
    // SQLite versions bundled with rusqlite, so the path is inlined. The path
    // is internally generated (a UUID in the app-data backups folder) and any
    // single quote is doubled, so a filesystem name cannot escape the literal.
    let escaped = target.display().to_string().replace('\'', "''");
    connection
        .execute_batch(&format!("VACUUM INTO '{escaped}'"))
        .map_err(|error| {
            let _ = fs::remove_file(&target);
            BackupError::Create(error.to_string())
        })?;
    let schema_version = read_schema_version(connection)?;
    let checksum = sha256_of_file(&target).map_err(|error| {
        let _ = fs::remove_file(&target);
        error
    })?;
    // Verify the backup is usable before claiming success. A backup that
    // fails its own integrity check is deleted so retention never promotes
    // a corrupt file into a restore candidate.
    if let Err(error) = integrity_check(&target) {
        let _ = fs::remove_file(&target);
        return Err(error);
    }
    let manifest = BackupManifest {
        id: id.clone(),
        kind,
        schema_version,
        created_at: now.to_owned(),
        app_version: app_version.to_owned(),
        checksum,
    };
    write_manifest(dir, &manifest)?;
    Ok(manifest)
}

/// Reads every manifest in the backups folder, newest first. Unreadable or
/// truncated manifests are skipped silently: they are not valid backups and
/// must never appear as restore candidates.
pub fn read_manifests(dir: &Path) -> Vec<BackupManifest> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut manifests = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        if let Ok(manifest) = serde_json::from_slice::<BackupManifest>(&bytes) {
            // A manifest without its database file is inert: drop it from the
            // list rather than surfacing a restore that cannot run.
            if backup_db_path(dir, &manifest.id).exists() {
                manifests.push(manifest);
            }
        }
    }
    manifests.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    manifests
}

/// Lists the valid backups in the folder, newest first. A backup is valid
/// when its checksum matches, it passes `PRAGMA integrity_check`, and its
/// schema version is no newer than this Nodepad supports. Invalid backups
/// are left on disk (they may still be recovered by hand) but never offered
/// to the thinker.
pub fn list_valid_backups(dir: &Path) -> Vec<BackupManifest> {
    read_manifests(dir)
        .into_iter()
        .filter(|manifest| validate_backup(dir, manifest).is_ok())
        .collect()
}

/// Validates one backup against its manifest: the file exists, its sha256
/// matches the manifest checksum, its `PRAGMA integrity_check` returns `ok`,
/// and its on-disk schema version matches the manifest and is no newer than
/// [`SUPPORTED_SCHEMA_VERSION`]. Any mismatch fails closed; the current
/// database is never touched by validation.
pub fn validate_backup(dir: &Path, manifest: &BackupManifest) -> Result<(), BackupError> {
    let db = backup_db_path(dir, &manifest.id);
    if !db.exists() {
        return Err(BackupError::Missing);
    }
    let actual = sha256_of_file(&db)?;
    if actual != manifest.checksum {
        return Err(BackupError::ChecksumMismatch);
    }
    integrity_check(&db)?;
    let on_disk_schema = read_schema_version_path(&db)?;
    if on_disk_schema != manifest.schema_version {
        return Err(BackupError::Manifest);
    }
    if manifest.schema_version > SUPPORTED_SCHEMA_VERSION {
        return Err(BackupError::UnsupportedSchema);
    }
    Ok(())
}

/// Enforces retention for one kind: keeps the newest backups up to that kind's
/// limit and deletes the rest, including their manifest sidecars. Older
/// backups are identified by `created_at` order, which is fixed-width UTC so
/// lexicographic order is chronological order.
pub fn retain(dir: &Path, kind: BackupKind) -> Result<(), BackupError> {
    let mut of_kind: Vec<BackupManifest> = read_manifests(dir)
        .into_iter()
        .filter(|manifest| manifest.kind == kind)
        .collect();
    // `read_manifests` already returns newest first.
    let limit = kind.retention_limit();
    if of_kind.len() <= limit {
        return Ok(());
    }
    for manifest in of_kind.drain(limit..) {
        let _ = fs::remove_file(backup_db_path(dir, &manifest.id));
        let _ = fs::remove_file(manifest_path(dir, &manifest.id));
    }
    Ok(())
}

/// A content fingerprint of the open database: a sha256 over every row of
/// every user table, enumerated from `sqlite_master` so a future migration
/// that adds a table is detected without editing this function. The
/// fingerprint changes when any durable data changes (including
/// `app_preferences` values such as the active Workspace) and is stable when
/// nothing has changed.
///
/// The whole-table scan is the cost of not instrumenting every write site;
/// Nodepad is a personal tool with modest data, and the automatic-backup
/// check only pays this cost on the first change of a local calendar day.
pub fn data_fingerprint(connection: &Connection) -> Result<String, BackupError> {
    let mut tables: Vec<String> = connection
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .map_err(|error| BackupError::Create(error.to_string()))?
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| BackupError::Create(error.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| BackupError::Create(error.to_string()))?;
    // FTS5 shadow tables are derived from `notes`; hashing them only doubles
    // the signal `notes` already provides and would couple the fingerprint to
    // the FTS implementation. Drop them.
    tables.retain(|name| !is_fts_shadow_table(name));
    let mut hasher = Sha256::new();
    for name in &tables {
        hasher.update(name.as_bytes());
        hasher.update(b"\x00");
        // `rowid` orders every table deterministically. `SELECT *` column
        // order is stable per schema. Values are rendered through SQLite's
        // own text conversion so the digest is independent of Rust's
        // formatting.
        let sql = format!("SELECT * FROM \"{name}\" ORDER BY rowid");
        let mut statement = connection
            .prepare(&sql)
            .map_err(|error| BackupError::Create(error.to_string()))?;
        let column_count = statement.column_count();
        let mut rows = statement
            .query([])
            .map_err(|error| BackupError::Create(error.to_string()))?;
        while let Ok(Some(row)) = rows.next() {
            for index in 0..column_count {
                let value: rusqlite::types::Value = row
                    .get(index)
                    .map_err(|error| BackupError::Create(error.to_string()))?;
                hasher.update(render_value(value).as_bytes());
                hasher.update(b"\x01");
            }
            hasher.update(b"\x02");
        }
    }
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&hasher.finalize());
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Replaces the live database at `db_path` with the backup identified by
/// `manifest`. The current database is preserved on any failure: it is
/// checkpointed, its stale WAL/SHM sidecars are removed, and the main file is
/// relocated (not deleted) before the verified backup is swapped in. If the
/// swap fails, the relocated file is moved back. The caller must have already
/// closed every connection to `db_path`.
pub fn replace_database(
    dir: &Path,
    manifest: &BackupManifest,
    db_path: &Path,
) -> Result<(), BackupError> {
    let backup_db = backup_db_path(dir, &manifest.id);
    if !backup_db.exists() {
        return Err(BackupError::Missing);
    }
    // Verify the source backup before touching the live file.
    let source_checksum = sha256_of_file(&backup_db)?;
    if source_checksum != manifest.checksum {
        return Err(BackupError::ChecksumMismatch);
    }

    // Flush any committed pages still in the WAL into the main file so the
    // relocated current database is complete if we have to roll back. A failed
    // checkpoint is best-effort: the live database may already be unopenable
    // in a recovery scenario, and we are about to replace it regardless.
    let _ = checkpoint(db_path);

    let wal = sidecar(db_path, "wal");
    let shm = sidecar(db_path, "shm");
    let _ = fs::remove_file(&wal);
    let _ = fs::remove_file(&shm);

    let staging = db_path.with_extension("nodepad_replacing");
    // If a previous interrupted restore left a staging file, clear it.
    let _ = fs::remove_file(&staging);

    if db_path.exists() {
        fs::rename(db_path, &staging).map_err(|error| BackupError::Replace(error.to_string()))?;
    }

    let tmp = db_path.with_extension("nodepad_restore_tmp");
    fs::copy(&backup_db, &tmp).map_err(|error| {
        rollback_to_live(&staging, db_path);
        BackupError::Replace(error.to_string())
    })?;
    let copied = sha256_of_file(&tmp).map_err(|error| {
        let _ = fs::remove_file(&tmp);
        rollback_to_live(&staging, db_path);
        error
    })?;
    if copied != manifest.checksum {
        let _ = fs::remove_file(&tmp);
        rollback_to_live(&staging, db_path);
        return Err(BackupError::ChecksumMismatch);
    }
    fs::rename(&tmp, db_path).map_err(|error| {
        rollback_to_live(&staging, db_path);
        BackupError::Replace(error.to_string())
    })?;

    // Success: the previous database is no longer needed.
    let _ = fs::remove_file(&staging);
    Ok(())
}

fn rollback_to_live(staging: &Path, db_path: &Path) {
    if staging.exists() {
        let _ = fs::rename(staging, db_path);
    }
}

fn sidecar(db_path: &Path, suffix: &str) -> PathBuf {
    let mut name = db_path.file_name().map(|name| name.to_os_string()).unwrap_or_default();
    name.push(format!("-{suffix}"));
    db_path.with_file_name(name)
}

fn checkpoint(db_path: &Path) -> Result<(), BackupError> {
    if !db_path.exists() {
        return Ok(());
    }
    let connection = Connection::open(db_path)
        .map_err(|error| BackupError::Reopen(error.to_string()))?;
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(|error| BackupError::Reopen(error.to_string()))?;
    Ok(())
}

fn write_manifest(dir: &Path, manifest: &BackupManifest) -> Result<(), BackupError> {
    let path = manifest_path(dir, &manifest.id);
    let bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|_| BackupError::Manifest)?;
    fs::write(&path, bytes).map_err(|_| BackupError::Manifest)
}

fn sha256_of_file(path: &Path) -> Result<String, BackupError> {
    let mut file = fs::File::open(path).map_err(|error| BackupError::Read(error.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65_536];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| BackupError::Read(error.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let bytes = hasher.finalize();
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn integrity_check(db_path: &Path) -> Result<(), BackupError> {
    // A file that exists and matches its checksum but cannot be opened as a
    // SQLite database is corrupt, not merely unreadable: the bytes are there,
    // they just do not form a usable database.
    let connection = Connection::open(db_path)
        .map_err(|_| BackupError::Corrupt)?;
    let result: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|_| BackupError::Corrupt)?;
    if result == "ok" {
        Ok(())
    } else {
        Err(BackupError::Corrupt)
    }
}

/// The highest applied migration version, read from a live connection. When
/// `schema_migrations` does not exist yet (a database from before migration
/// tracking, or one that migration 1 has not run against), this reports zero
/// so the open path treats every migration as pending.
fn read_schema_version(connection: &Connection) -> Result<i64, BackupError> {
    read_schema_version_inner(connection)
}

fn read_schema_version_path(db_path: &Path) -> Result<i64, BackupError> {
    let connection = Connection::open(db_path)
        .map_err(|error| BackupError::Read(error.to_string()))?;
    read_schema_version_inner(&connection)
}

fn read_schema_version_inner(connection: &Connection) -> Result<i64, BackupError> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| BackupError::Read(error.to_string()))?;
    if !exists {
        return Ok(0);
    }
    let version: Option<i64> = connection
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| row.get(0))
        .ok();
    Ok(version.unwrap_or(0))
}

/// Renders a SQLite value to a stable, formatting-independent string for the
/// fingerprint. `rusqlite::types::Value` does not implement `Display`, so the
/// digest is produced from this canonical rendering instead.
fn render_value(value: rusqlite::types::Value) -> String {
    use rusqlite::types::Value;
    match value {
        Value::Null => "\u{2205}".to_owned(),
        Value::Integer(integer) => format!("i:{integer}"),
        Value::Real(real) => format!("r:{real}"),
        Value::Text(text) => format!("t:{text}"),
        Value::Blob(bytes) => format!("b:{}", hex(&bytes)),
    }
}

/// Lowercase hex of a byte slice, without pulling in another dependency.
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn is_fts_shadow_table(name: &str) -> bool {
    // FTS5 names a virtual table `<base>` and shadow tables `<base>_data`,
    // `<base>_idx`, `<base>_content`, `<base>_config`, `<base>_docsize`.
    // The only FTS table Nodepad ships is `note_search`; its content is derived
    // entirely from `notes`, so hashing the virtual table or any shadow table
    // only doubles the signal `notes` already provides.
    name == "note_search" || name.starts_with("note_search_")
}

#[cfg(test)]
mod tests;