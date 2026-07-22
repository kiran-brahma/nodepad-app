## Parent

Part of #1.

## What to build

Add local rotating backup, restore, and migration safety for SQLite. Backups live in the macOS application-data area, use SQLite-safe backup semantics, and are validated before restore. No network backup exists.

## Decisions

- Before every schema migration, create a verified pre-migration backup when a database exists.
- Automatic backup runs at most once per local calendar day after durable data changed since the last backup.
- Retain the latest seven valid automatic backups plus explicit pre-migration/pre-restore backups governed by documented cleanup.
- Use SQLite backup API or `VACUUM INTO` equivalent safe semantics; never copy a live WAL database naively.
- Store manifest metadata: schema version, created time, app version, checksum, and backup kind.
- Restore requires explicit confirmation, validates manifest/checksum and database integrity, creates a pre-restore backup, closes active handles, replaces atomically, and reloads durable state.
- Invalid backup never replaces current data.
- Recovery screen can list valid local backups and restore one.

## Acceptance criteria

- [ ] A changed database produces at most one automatic backup per day.
- [ ] Unchanged data does not create redundant daily backups.
- [ ] Retention keeps the intended seven automatic backups deterministically.
- [ ] Every migration is preceded by a valid backup or aborts without migration.
- [ ] Restore validates integrity/checksum/schema before confirmation and replacement.
- [ ] Restore creates a recoverable pre-restore backup and reopened state matches the selected backup.
- [ ] Injected backup/restore failures preserve current database and present typed recovery state.
- [ ] No backup is uploaded, encrypted with an app password, or placed outside app data without an explicit later feature.

## Testing decisions

- Use a deterministic clock and temporary app-data directory.
- Cover WAL-active backup, retention, unchanged day, checksum mismatch, corrupt SQLite, unsupported schema, failed pre-restore backup, failed replacement, and successful reopen.
- Test migration backup ordering and abort behavior.

## Blocked by

- #3

## Scope fence

Do not add Time Machine integration, cloud/network backup, scheduled notifications, archive export, encryption, or full event/version history.

## Delivery workflow

Run `prd-simplicity-audit`, use a fresh `implement` session, then `code-review`, scoped `fallow`, focused tests and repository gates, and one PR against `main`.
