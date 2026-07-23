# PRD SIMPLICITY AUDIT

Feature: V0-16 — Rotating backup, restore, and migration safety
Issue: kiran-brahma/nodepad-app#17
Date: today
Gate: **PROCEED**

---

## MODULE MAP

### Existing modules (will be extended)

- `src-tauri/src/workspace.rs` — durable Thinking Workspace state. Owns the
  SQLite connection, `migrate()`, `WorkspaceStore::open` / `open_prepared` /
  `open_at`, `StorageOpenFailure`, and `app_preferences`. The migrations list
  (versions 1–9) is embedded in `migrate()`.
- `src-tauri/src/lib.rs` — Tauri command surface and `AppState` wiring. Owns
  `open_storage(app)`, the `storage: Mutex<Result<WorkspaceStore, StorageOpenFailure>>`,
  and the `app_data_dir` lookup. The recovery command `retry_storage_open`
  and the storage-recovery UI seam live here.
- `src/storage-recovery.tsx` — the screen shown when storage would not open.
  Today it offers retry/quit only.
- `src/workspace-client.ts` — the UI's only durable-state seam; every Tauri
  command binding and typed outcome lives here.

### New modules

- `src-tauri/src/backup.rs` — owns SQLite-safe backup semantics, the manifest
  format, checksums, integrity validation, retention, and atomic file
  replacement. Connection-agnostic: every function takes `&Connection` or a
  path. This is where "never copy a live WAL database naively" lives, once.

---

## INTERROGATION FINDINGS

### Pre-migration backup before every migration

**CAUTION → resolved.** The first instinct is to teach `migrate()` about
backups, which would complect schema migration with the backup folder, the
clock, and the app version. Instead, migration stays pure: a new
`pending_migration_count(connection)` reads `schema_migrations` against the
shared migrations list, and the open path creates the verified pre-migration
backup only when migrations are actually pending and the database already
exists. `migrate()` changes only by exposing its migrations list to the
pending-count helper. **CLEAN after resolution.**

### Automatic backup once per local calendar day after durable data changed

**CLEAN.** The new state is two `app_preferences` rows —
`backup.last_automatic_day` and `backup.last_fingerprint` — so no new table
or migration is introduced. The fingerprint is a full-scan hash over every
user table enumerated from `sqlite_master`, so a future migration that adds a
table is detected automatically instead of requiring a forgotten edit here.
The fingerprint and the day gate live in one method on `WorkspaceStore`; the
backup bytes come from `backup::create_backup`. State is isolated behind the
backup module + one store method.

### Retention of seven automatic backups plus explicit backups

**CLEAN.** Retention is one function per kind, called right after a backup of
that kind is written. Automatic backups are capped at seven; pre-migration
and pre-restore backups are capped at four each (documented in the module), so
explicit backups cannot grow without bound while never competing with the
seven automatic slots.

### Restore: validate, pre-restore backup, close, replace, reload

**CAUTION → resolved.** Restore is inherently stateful (it must close the live
connection and reopen). The complexity is pulled into one Tauri command that
holds the storage mutex across the whole operation: validate the selected
backup read-only, make a pre-restore backup from the live connection, drop the
store to close the handle, replace files via `backup::replace_database`, then
reopen with the backup-aware open so an older-schema backup migrates forward
behind its own pre-migration backup. `backup::replace_database` preserves the
current database on any failure by checkpointing and relocating the live file
before attempting the swap. **CLEAN after resolution.**

### Invalid backup never replaces current data; recovery screen lists valid backups

**CLEAN.** `list_backups` validates every manifest (checksum + integrity +
supported schema) and returns only valid records, so the recovery screen
never offers an invalid backup. `restore_backup` re-validates before any
durable mutation. Both commands operate on the backups directory and the
database path, so they work even when `AppState.storage` is `Err` — exactly
the recovery scenario.

---

## COMPLEXITY SCORECARD

State Surface: **Low** — two `app_preferences` rows and manifest sidecar files,
all owned by the backup module.
Seam Quality: **Preserved** — `migrate()` stays pure; backup is a new deep
module; the UI seam stays the single `workspace-client.ts`.
Module Cohesion: **Cohesive** — `backup.rs` has one responsibility and hides
the SQLite-safe semantics, manifest format, and retention policy.
Change Blast Radius: **Narrow** — backup policy changes live in `backup.rs`;
retention counts and the fingerprint are constants/functions in one module.
Incidental Complexity Load: **Mostly Problem** — the statefulness of restore
is intrinsic to the feature, not introduced by the implementation.

Summary: The PRD is structurally sound. Two cautions (migration/backup
coupling and restore statefulness) are resolved by keeping `migrate()` pure
and concentrating restore orchestration in one command that delegates file
safety to `backup.rs`. No existing clean seam is crossed.

---

## GATE DECISION: PROCEED

Hand `backup.rs`, the open-path pre-migration hook, the automatic-backup
method, the restore command, the TS client bindings, and the recovery-screen
restore affordance to implementation.