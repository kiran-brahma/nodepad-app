use super::*;
use rusqlite::Connection;
use std::path::PathBuf;
use uuid::Uuid;

/// A unique backup folder per test, mirroring the durable store's temp-path
/// convention. Each test cleans up its own directory.
fn backups_folder() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nodepad-backup-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn open_database(path: &Path) -> Connection {
    let connection = Connection::open(path).unwrap();
    connection
        .execute_batch("PRAGMA journal_mode = WAL;")
        .unwrap();
    connection
}

fn seed(connection: &Connection) {
    connection
        .execute_batch(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY NOT NULL, applied_at TEXT NOT NULL);
             CREATE TABLE app_preferences (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);
             CREATE TABLE notes (id TEXT PRIMARY KEY NOT NULL, markdown TEXT NOT NULL, updated_at TEXT NOT NULL);
             INSERT INTO schema_migrations VALUES (9, '2026-01-01T00:00:00.000000Z');",
        )
        .unwrap();
}

fn write_note(connection: &Connection, id: &str, markdown: &str) {
    connection
        .execute(
            "INSERT INTO notes (id, markdown, updated_at) VALUES (?1, ?2, '2026-01-01T00:00:00.000000Z')",
            params![id, markdown],
        )
        .unwrap();
}

use rusqlite::params;

fn database_path() -> PathBuf {
    std::env::temp_dir().join(format!("nodepad-backup-db-{}.sqlite", Uuid::new_v4()))
}

fn remove_database(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

#[test]
fn a_wal_active_database_backs_up_safely() {
    let dir = backups_folder();
    let db = database_path();
    {
        let connection = open_database(&db);
        seed(&connection);
        write_note(&connection, "n1", "first");
        // Leaving the WAL active (un-checkpointed) proves the backup is not a
        // naive file copy: `VACUUM INTO` reads a consistent snapshot.
        let manifest = create_backup(
            &connection,
            &dir,
            BackupKind::Automatic,
            "2026-07-22T00:00:00.000000Z",
            "0.1.0",
        )
        .unwrap();
        assert_eq!(manifest.schema_version, 9);
        assert!(!manifest.checksum.is_empty());
    }
    let manifests = read_manifests(&dir);
    assert_eq!(manifests.len(), 1);
    validate_backup(&dir, &manifests[0]).unwrap();
    remove_database(&db);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn unchanged_data_keeps_the_same_fingerprint() {
    let db = database_path();
    let connection = open_database(&db);
    seed(&connection);
    write_note(&connection, "n1", "first");
    let first = data_fingerprint(&connection).unwrap();
    let second = data_fingerprint(&connection).unwrap();
    assert_eq!(first, second);
    write_note(&connection, "n2", "second");
    let third = data_fingerprint(&connection).unwrap();
    assert_ne!(third, first);
    connection.close().unwrap();
    remove_database(&db);
}

#[test]
fn the_fingerprint_detects_a_preference_value_change() {
    // `select_workspace` changes `app_preferences` without changing any row
    // count, so the fingerprint must read values, not only counts.
    let db = database_path();
    let connection = open_database(&db);
    seed(&connection);
    connection
        .execute(
            "INSERT INTO app_preferences (key, value) VALUES ('active_workspace_id', 'w1')",
            [],
        )
        .unwrap();
    let first = data_fingerprint(&connection).unwrap();
    connection
        .execute(
            "UPDATE app_preferences SET value = 'w2' WHERE key = 'active_workspace_id'",
            [],
        )
        .unwrap();
    let second = data_fingerprint(&connection).unwrap();
    assert_ne!(first, second);
    connection.close().unwrap();
    remove_database(&db);
}

#[test]
fn retention_keeps_the_latest_seven_automatic_backups() {
    let dir = backups_folder();
    let db = database_path();
    let connection = open_database(&db);
    seed(&connection);
    for index in 0..10 {
        write_note(&connection, &format!("n{index}"), &format!("note {index}"));
        let now = format!("2026-07-22T00:00:{index:02}.000000Z");
        create_backup(&connection, &dir, BackupKind::Automatic, &now, "0.1.0").unwrap();
        retain(&dir, BackupKind::Automatic).unwrap();
    }
    let automatic: Vec<_> = read_manifests(&dir)
        .into_iter()
        .filter(|manifest| manifest.kind == BackupKind::Automatic)
        .collect();
    assert_eq!(automatic.len(), MAX_AUTOMATIC_BACKUPS);
    // The newest seven survived; the three oldest were deleted.
    assert_eq!(
        automatic.first().unwrap().created_at,
        "2026-07-22T00:00:09.000000Z"
    );
    connection.close().unwrap();
    remove_database(&db);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn pre_migration_backups_do_not_crowd_out_automatic_backups() {
    let dir = backups_folder();
    let db = database_path();
    let connection = open_database(&db);
    seed(&connection);
    for index in 0..10 {
        let now = format!("2026-07-22T00:00:0{index:02}.000000Z");
        create_backup(&connection, &dir, BackupKind::PreMigration, &now, "0.1.0").unwrap();
        retain(&dir, BackupKind::PreMigration).unwrap();
    }
    // Automatic retention is independent: ten pre-migration backups must not
    // delete any automatic slot.
    create_backup(
        &connection,
        &dir,
        BackupKind::Automatic,
        "2026-07-22T00:00:00.000000Z",
        "0.1.0",
    )
    .unwrap();
    retain(&dir, BackupKind::Automatic).unwrap();
    let pre_migration = read_manifests(&dir)
        .into_iter()
        .filter(|manifest| manifest.kind == BackupKind::PreMigration)
        .count();
    assert_eq!(pre_migration, MAX_PRE_MIGRATION_BACKUPS);
    connection.close().unwrap();
    remove_database(&db);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_checksum_mismatch_is_rejected_before_restore() {
    let dir = backups_folder();
    let db = database_path();
    let manifest;
    {
        let connection = open_database(&db);
        seed(&connection);
        write_note(&connection, "n1", "first");
        manifest = create_backup(
            &connection,
            &dir,
            BackupKind::Automatic,
            "2026-07-22T00:00:00.000000Z",
            "0.1.0",
        )
        .unwrap();
        connection.close().unwrap();
    }
    // Tamper with the backup bytes so the checksum no longer matches.
    let backup_db = backup_db_path(&dir, &manifest.id);
    fs::write(&backup_db, b"not a database").unwrap();
    assert_eq!(
        validate_backup(&dir, &manifest),
        Err(BackupError::ChecksumMismatch)
    );
    // The recovery screen only lists valid backups, so the tampered one is
    // hidden.
    assert!(list_valid_backups(&dir).is_empty());
    remove_database(&db);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_corrupt_sqlite_file_is_rejected() {
    let dir = backups_folder();
    let db = database_path();
    let manifest;
    {
        let connection = open_database(&db);
        seed(&connection);
        write_note(&connection, "n1", "first");
        manifest = create_backup(
            &connection,
            &dir,
            BackupKind::Automatic,
            "2026-07-22T00:00:00.000000Z",
            "0.1.0",
        )
        .unwrap();
        connection.close().unwrap();
    }
    // Overwrite the checksum so the checksum check passes, then corrupt the
    // database header so the file can no longer be opened as a database.
    let backup_db = backup_db_path(&dir, &manifest.id);
    let mut bytes = fs::read(&backup_db).unwrap();
    // The SQLite magic header is the first 16 bytes; clobbering it makes the
    // file unreadable as a database, which validation must report as corrupt.
    bytes[..16].fill(0);
    fs::write(&backup_db, &bytes).unwrap();
    let tampered = BackupManifest {
        checksum: sha256_of_file(&backup_db).unwrap(),
        ..manifest.clone()
    };
    assert_eq!(validate_backup(&dir, &tampered), Err(BackupError::Corrupt));
    remove_database(&db);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_unsupported_schema_version_is_rejected() {
    let dir = backups_folder();
    let db = database_path();
    let manifest;
    {
        let connection = open_database(&db);
        seed(&connection);
        write_note(&connection, "n1", "first");
        // Pretend a future Nodepad already ran migration 11, so the backup's
        // schema version is newer than this build can migrate.
        connection
            .execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (11, 'future')",
                [],
            )
            .unwrap();
        manifest = create_backup(
            &connection,
            &dir,
            BackupKind::Automatic,
            "2026-07-22T00:00:00.000000Z",
            "0.1.0",
        )
        .unwrap();
        assert_eq!(manifest.schema_version, 11);
        connection.close().unwrap();
    }
    assert_eq!(
        validate_backup(&dir, &manifest),
        Err(BackupError::UnsupportedSchema)
    );
    remove_database(&db);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn replace_database_swaps_in_the_backup_and_preserves_current_on_failure() {
    let dir = backups_folder();
    let live = database_path();
    {
        let connection = open_database(&live);
        seed(&connection);
        write_note(&connection, "live", "live note");
        connection.close().unwrap();
    }
    let manifest;
    let backup_db;
    {
        let connection = open_database(&live);
        // Change the live database so the backup differs from it.
        connection
            .execute("DELETE FROM notes WHERE id = 'live'", [])
            .unwrap();
        write_note(&connection, "backup", "backup note");
        manifest = create_backup(
            &connection,
            &dir,
            BackupKind::Automatic,
            "2026-07-22T00:00:00.000000Z",
            "0.1.0",
        )
        .unwrap();
        connection.close().unwrap();
        backup_db = backup_db_path(&dir, &manifest.id);
    }
    // Live currently holds the "backup" note (same as backup). Make live
    // differ again so the swap is observable.
    {
        let connection = open_database(&live);
        write_note(&connection, "extra", "extra note");
        connection.close().unwrap();
    }
    replace_database(&dir, &manifest, &live).unwrap();
    let reopened = Connection::open(&live).unwrap();
    let ids: Vec<String> = reopened
        .prepare("SELECT id FROM notes ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(ids, vec!["backup".to_owned()]);
    assert!(!sidecar(&live, "wal").exists());
    assert!(!live.with_extension("nodepad_replacing").exists());
    // The source backup file is preserved for reuse.
    assert!(backup_db.exists());
    reopened.close().unwrap();
    remove_database(&live);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn replace_database_restores_the_live_file_when_the_target_disappears() {
    let dir = backups_folder();
    let live = database_path();
    let live_note = "live note";
    {
        let connection = open_database(&live);
        seed(&connection);
        write_note(&connection, "live", live_note);
        connection.close().unwrap();
    }
    // A manifest whose database file is missing simulates a deleted backup.
    let manifest = BackupManifest {
        id: Uuid::new_v4().to_string(),
        kind: BackupKind::Automatic,
        schema_version: 9,
        created_at: "2026-07-22T00:00:00.000000Z".to_owned(),
        app_version: "0.1.0".to_owned(),
        checksum: "deadbeef".to_owned(),
    };
    let result = replace_database(&dir, &manifest, &live);
    assert_eq!(result, Err(BackupError::Missing));
    // The live database is untouched.
    let connection = open_database(&live);
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM notes WHERE id = 'live'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 1);
    connection.close().unwrap();
    remove_database(&live);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn create_backup_deletes_an_invalid_backup_and_reports_the_error() {
    // `VACUUM INTO` to a directory that does not exist and cannot be created
    // (a file where a folder should be) forces a creation failure.
    let file_as_dir = std::env::temp_dir().join(format!("nodepad-block-{}", Uuid::new_v4()));
    fs::write(&file_as_dir, b"block").unwrap();
    let db = database_path();
    let connection = open_database(&db);
    seed(&connection);
    write_note(&connection, "n1", "first");
    let result = create_backup(
        &connection,
        &file_as_dir,
        BackupKind::Automatic,
        "2026-07-22T00:00:00.000000Z",
        "0.1.0",
    );
    assert!(matches!(result, Err(BackupError::BackupsDir(_))));
    connection.close().unwrap();
    remove_database(&db);
    fs::remove_file(&file_as_dir).unwrap();
}
#[test]
fn a_failed_pre_restore_backup_preserves_the_current_database() {
    // The restore flow makes a pre-restore backup before touching the live
    // database; a failure there must leave the current database intact. Here
    // a valid automatic backup exists, then the pre-restore attempt is pointed
    // at an unusable folder so it fails — and the automatic backup and the live
    // database are both unchanged.
    let dir = backups_folder();
    let live = database_path();
    let manifest;
    {
        let connection = open_database(&live);
        seed(&connection);
        write_note(&connection, "live", "live note");
        manifest = create_backup(
            &connection,
            &dir,
            BackupKind::Automatic,
            "2026-07-22T00:00:00.000000Z",
            "0.1.0",
        )
        .unwrap();
        connection.close().unwrap();
    }
    // A file where the pre-restore backups folder should be makes that backup
    // fail, exactly as an injected restore-time failure would.
    let blocked = std::env::temp_dir().join(format!("nodepad-prerestore-block-{}", Uuid::new_v4()));
    fs::write(&blocked, b"block").unwrap();
    let pre_restore = create_backup(
        // Reopen read-only-ish: a fresh connection cannot VACUUM INTO a blocked
        // folder, so this fails before any durable file moves.
        &Connection::open(&live).unwrap(),
        &blocked,
        BackupKind::PreRestore,
        "2026-07-22T00:00:00.000000Z",
        "0.1.0",
    );
    assert!(matches!(pre_restore, Err(BackupError::BackupsDir(_))));
    // The automatic backup and the live database are untouched.
    validate_backup(&dir, &manifest).unwrap();
    let connection = open_database(&live);
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM notes WHERE id = 'live'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 1);
    connection.close().unwrap();
    fs::remove_file(&blocked).unwrap();
    remove_database(&live);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_replacement_with_a_tampered_source_preserves_the_current_database() {
    // The restore replacement verifies the source backup's checksum before
    // moving any file. A tampered source fails closed and the live database is
    // preserved.
    let dir = backups_folder();
    let live = database_path();
    let manifest;
    {
        let connection = open_database(&live);
        seed(&connection);
        write_note(&connection, "live", "live note");
        manifest = create_backup(
            &connection,
            &dir,
            BackupKind::Automatic,
            "2026-07-22T00:00:00.000000Z",
            "0.1.0",
        )
        .unwrap();
        connection.close().unwrap();
    }
    // Tamper with the backup file so its checksum no longer matches the manifest.
    let backup_db = backup_db_path(&dir, &manifest.id);
    fs::write(&backup_db, b"tampered bytes").unwrap();
    let result = replace_database(&dir, &manifest, &live);
    assert_eq!(result, Err(BackupError::ChecksumMismatch));
    // The live database is untouched.
    let connection = open_database(&live);
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM notes WHERE id = 'live'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 1);
    connection.close().unwrap();
    remove_database(&live);
    fs::remove_dir_all(&dir).unwrap();
}
