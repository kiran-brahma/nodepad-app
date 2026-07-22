use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

const DEFAULT_WORKSPACE_NAME: &str = "My Thinking Workspace";
/// Names are bounded in Unicode scalar values, never bytes or grapheme clusters.
const MAX_WORKSPACE_NAME_SCALARS: usize = 120;
const ACTIVE_WORKSPACE_PREFERENCE: &str = "active_workspace_id";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingWorkspace {
    id: String,
    name: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    id: String,
    workspace_id: String,
    markdown: String,
    note_type: String,
    created_at: String,
    updated_at: String,
    pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    workspaces: Vec<ThinkingWorkspace>,
    notes: Vec<Note>,
    active_workspace_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceFailureCode {
    Validation,
    NotFound,
    Storage,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkspaceFailure {
    code: WorkspaceFailureCode,
    message: String,
}

/// Why durable storage could not be opened, so recovery can name the category.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageOpenFailureCategory {
    Unreadable,
    Migration,
    Initialization,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageOpenFailure {
    category: StorageOpenFailureCategory,
    message: String,
}

impl StorageOpenFailure {
    pub fn new(category: StorageOpenFailureCategory, message: impl Into<String>) -> Self {
        Self {
            category,
            message: message.into(),
        }
    }

    fn from_error(category: StorageOpenFailureCategory, error: WorkspaceError) -> Self {
        Self::new(category, error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum WorkspaceCommandResult {
    Committed { snapshot: WorkspaceSnapshot },
    Failed { failure: WorkspaceFailure },
    Unavailable { failure: StorageOpenFailure },
}

#[derive(Debug, Error)]
pub(crate) enum WorkspaceError {
    #[error("A Thinking Workspace name is required.")]
    EmptyWorkspaceName,
    #[error("A Thinking Workspace name may not exceed {MAX_WORKSPACE_NAME_SCALARS} characters.")]
    WorkspaceNameTooLong,
    #[error("A Note needs Markdown text.")]
    EmptyNote,
    #[error("The selected Thinking Workspace no longer exists.")]
    WorkspaceNotFound,
    #[error("Local storage could not commit this change. Please try again.")]
    Storage(#[source] rusqlite::Error),
}

impl WorkspaceError {
    fn failure(&self) -> WorkspaceFailure {
        let code = match self {
            Self::EmptyWorkspaceName | Self::WorkspaceNameTooLong | Self::EmptyNote => {
                WorkspaceFailureCode::Validation
            }
            Self::WorkspaceNotFound => WorkspaceFailureCode::NotFound,
            Self::Storage(_) => WorkspaceFailureCode::Storage,
        };
        WorkspaceFailure {
            code,
            message: self.to_string(),
        }
    }
}

/// The sole interface for durable Thinking Workspace intents and committed state.
pub trait ThinkingWorkspaceInterface {
    fn snapshot(&self) -> Result<WorkspaceSnapshot, WorkspaceError>;
    fn create_workspace(&mut self, name: &str) -> Result<WorkspaceSnapshot, WorkspaceError>;
    fn select_workspace(&mut self, workspace_id: &str)
        -> Result<WorkspaceSnapshot, WorkspaceError>;
    fn rename_workspace(
        &mut self,
        workspace_id: &str,
        name: &str,
    ) -> Result<WorkspaceSnapshot, WorkspaceError>;
    fn delete_workspace(&mut self, workspace_id: &str)
        -> Result<WorkspaceSnapshot, WorkspaceError>;
    fn create_note(
        &mut self,
        workspace_id: &str,
        markdown: &str,
    ) -> Result<WorkspaceSnapshot, WorkspaceError>;

    fn snapshot_outcome(&self) -> WorkspaceCommandResult {
        outcome(self.snapshot())
    }
    fn create_workspace_outcome(&mut self, name: &str) -> WorkspaceCommandResult {
        outcome(self.create_workspace(name))
    }
    fn select_workspace_outcome(&mut self, workspace_id: &str) -> WorkspaceCommandResult {
        outcome(self.select_workspace(workspace_id))
    }
    fn rename_workspace_outcome(
        &mut self,
        workspace_id: &str,
        name: &str,
    ) -> WorkspaceCommandResult {
        outcome(self.rename_workspace(workspace_id, name))
    }
    fn delete_workspace_outcome(&mut self, workspace_id: &str) -> WorkspaceCommandResult {
        outcome(self.delete_workspace(workspace_id))
    }
    fn create_note_outcome(
        &mut self,
        workspace_id: &str,
        markdown: &str,
    ) -> WorkspaceCommandResult {
        outcome(self.create_note(workspace_id, markdown))
    }
}

#[derive(Debug)]
pub struct WorkspaceStore {
    connection: Connection,
}

impl WorkspaceStore {
    /// Opens durable storage. A failure never resets, deletes, or overwrites an
    /// existing database; it reports the category so recovery can retry or quit.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageOpenFailure> {
        let mut connection = Connection::open(path).map_err(|error| {
            StorageOpenFailure::from_error(
                StorageOpenFailureCategory::Unreadable,
                WorkspaceError::Storage(error),
            )
        })?;
        migrate(&mut connection).map_err(|error| {
            StorageOpenFailure::from_error(StorageOpenFailureCategory::Migration, error)
        })?;
        let mut store = Self { connection };
        store.ensure_ready().map_err(|error| {
            StorageOpenFailure::from_error(StorageOpenFailureCategory::Initialization, error)
        })?;
        Ok(store)
    }

    /// Restores the invariant that one valid Workspace exists and is selected.
    fn ensure_ready(&mut self) -> Result<(), WorkspaceError> {
        if read_workspaces(&self.connection)?.is_empty() {
            self.create_workspace(DEFAULT_WORKSPACE_NAME)?;
        }
        let workspaces = read_workspaces(&self.connection)?;
        let active = read_active_workspace_id(&self.connection)?;
        let selected = active.filter(|id| workspaces.iter().any(|workspace| &workspace.id == id));
        if selected.is_none() {
            let fallback = workspaces
                .first()
                .ok_or(WorkspaceError::WorkspaceNotFound)?
                .id
                .clone();
            write_active_workspace_id(&self.connection, &fallback)?;
        }
        Ok(())
    }
}

impl ThinkingWorkspaceInterface for WorkspaceStore {
    fn snapshot(&self) -> Result<WorkspaceSnapshot, WorkspaceError> {
        read_snapshot(&self.connection)
    }

    fn create_workspace(&mut self, name: &str) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let name = validated_workspace_name(name)?;
        let now = timestamp();
        let workspace_id = id();
        let transaction = self
            .connection
            .transaction()
            .map_err(WorkspaceError::Storage)?;
        transaction.execute(
            "INSERT INTO thinking_workspaces (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            params![workspace_id, name, now],
        ).map_err(WorkspaceError::Storage)?;
        write_active_workspace_id(&transaction, &workspace_id)?;
        transaction.commit().map_err(WorkspaceError::Storage)?;
        self.snapshot()
    }

    fn select_workspace(
        &mut self,
        workspace_id: &str,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        require_workspace(&read_workspaces(&self.connection)?, workspace_id)?;
        let transaction = self
            .connection
            .transaction()
            .map_err(WorkspaceError::Storage)?;
        write_active_workspace_id(&transaction, workspace_id)?;
        transaction.commit().map_err(WorkspaceError::Storage)?;
        self.snapshot()
    }

    fn rename_workspace(
        &mut self,
        workspace_id: &str,
        name: &str,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let name = validated_workspace_name(name)?;
        require_workspace(&read_workspaces(&self.connection)?, workspace_id)?;
        let transaction = self
            .connection
            .transaction()
            .map_err(WorkspaceError::Storage)?;
        transaction
            .execute(
                "UPDATE thinking_workspaces SET name = ?2, updated_at = ?3 WHERE id = ?1",
                params![workspace_id, name, timestamp()],
            )
            .map_err(WorkspaceError::Storage)?;
        transaction.commit().map_err(WorkspaceError::Storage)?;
        self.snapshot()
    }

    fn delete_workspace(
        &mut self,
        workspace_id: &str,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let workspaces = read_workspaces(&self.connection)?;
        require_workspace(&workspaces, workspace_id)?;
        let survivor = surviving_workspace_id(&workspaces, workspace_id);
        let active = read_active_workspace_id(&self.connection)?;
        let transaction = self
            .connection
            .transaction()
            .map_err(WorkspaceError::Storage)?;
        let next_active = match &survivor {
            // Child Notes go with the Workspace through the schema's cascade.
            Some(survivor_id) => {
                transaction
                    .execute(
                        "DELETE FROM thinking_workspaces WHERE id = ?1",
                        params![workspace_id],
                    )
                    .map_err(WorkspaceError::Storage)?;
                match active.as_deref() {
                    Some(active_id) if active_id != workspace_id => active_id.to_owned(),
                    _ => survivor_id.clone(),
                }
            }
            // The last Workspace is emptied and reset instead of leaving none.
            None => {
                transaction
                    .execute(
                        "DELETE FROM notes WHERE workspace_id = ?1",
                        params![workspace_id],
                    )
                    .map_err(WorkspaceError::Storage)?;
                transaction
                    .execute(
                        "UPDATE thinking_workspaces SET name = ?2, updated_at = ?3 WHERE id = ?1",
                        params![workspace_id, DEFAULT_WORKSPACE_NAME, timestamp()],
                    )
                    .map_err(WorkspaceError::Storage)?;
                workspace_id.to_owned()
            }
        };
        write_active_workspace_id(&transaction, &next_active)?;
        transaction.commit().map_err(WorkspaceError::Storage)?;
        self.snapshot()
    }

    fn create_note(
        &mut self,
        workspace_id: &str,
        markdown: &str,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        if markdown.trim().is_empty() {
            return Err(WorkspaceError::EmptyNote);
        }
        require_workspace(&read_workspaces(&self.connection)?, workspace_id)?;
        let now = timestamp();
        let transaction = self
            .connection
            .transaction()
            .map_err(WorkspaceError::Storage)?;
        transaction.execute(
            "INSERT INTO notes (id, workspace_id, markdown, note_type, created_at, updated_at, pinned) VALUES (?1, ?2, ?3, 'general', ?4, ?4, 0)",
            params![id(), workspace_id, markdown, now],
        ).map_err(WorkspaceError::Storage)?;
        transaction.commit().map_err(WorkspaceError::Storage)?;
        self.snapshot()
    }
}

fn outcome(result: Result<WorkspaceSnapshot, WorkspaceError>) -> WorkspaceCommandResult {
    match result {
        Ok(snapshot) => WorkspaceCommandResult::Committed { snapshot },
        Err(error) => WorkspaceCommandResult::Failed {
            failure: error.failure(),
        },
    }
}

fn validated_workspace_name(name: &str) -> Result<String, WorkspaceError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(WorkspaceError::EmptyWorkspaceName);
    }
    if name.chars().count() > MAX_WORKSPACE_NAME_SCALARS {
        return Err(WorkspaceError::WorkspaceNameTooLong);
    }
    Ok(name.to_owned())
}

fn require_workspace(
    workspaces: &[ThinkingWorkspace],
    workspace_id: &str,
) -> Result<(), WorkspaceError> {
    workspaces
        .iter()
        .any(|workspace| workspace.id == workspace_id)
        .then_some(())
        .ok_or(WorkspaceError::WorkspaceNotFound)
}

/// The deterministic survivor of a delete: most recently updated, then most
/// recently created, then highest id, so every adapter picks the same one.
fn surviving_workspace_id(
    workspaces: &[ThinkingWorkspace],
    deleted_id: &str,
) -> Option<String> {
    workspaces
        .iter()
        .filter(|workspace| workspace.id != deleted_id)
        .max_by(|left, right| {
            (&left.updated_at, &left.created_at, &left.id).cmp(&(
                &right.updated_at,
                &right.created_at,
                &right.id,
            ))
        })
        .map(|workspace| workspace.id.clone())
}

fn migrate(connection: &mut Connection) -> Result<(), WorkspaceError> {
    connection
        .execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")
        .map_err(WorkspaceError::Storage)?;
    let migrations = [
        (1_i64, include_str!("../migrations/0001_initial.sql")),
        (2_i64, include_str!("../migrations/0002_preferences.sql")),
    ];
    for (version, sql) in migrations {
        let transaction = connection.transaction().map_err(WorkspaceError::Storage)?;
        transaction.execute_batch("CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY NOT NULL, applied_at TEXT NOT NULL);").map_err(WorkspaceError::Storage)?;
        let applied: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
                [version],
                |row| row.get(0),
            )
            .map_err(WorkspaceError::Storage)?;
        if !applied {
            transaction
                .execute_batch(sql)
                .map_err(WorkspaceError::Storage)?;
            transaction
                .execute(
                    "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                    params![version, timestamp()],
                )
                .map_err(WorkspaceError::Storage)?;
        }
        transaction.commit().map_err(WorkspaceError::Storage)?;
    }
    Ok(())
}

fn read_workspaces(connection: &Connection) -> Result<Vec<ThinkingWorkspace>, WorkspaceError> {
    connection
        .prepare(
            "SELECT id, name, created_at, updated_at FROM thinking_workspaces ORDER BY created_at",
        )
        .map_err(WorkspaceError::Storage)?
        .query_map([], |row| {
            Ok(ThinkingWorkspace {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })
        .map_err(WorkspaceError::Storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(WorkspaceError::Storage)
}

fn read_active_workspace_id(connection: &Connection) -> Result<Option<String>, WorkspaceError> {
    connection
        .query_row(
            "SELECT value FROM app_preferences WHERE key = ?1",
            [ACTIVE_WORKSPACE_PREFERENCE],
            |row| row.get(0),
        )
        .optional()
        .map_err(WorkspaceError::Storage)
}

fn write_active_workspace_id(
    connection: &Connection,
    workspace_id: &str,
) -> Result<(), WorkspaceError> {
    connection
        .execute(
            "INSERT INTO app_preferences (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![ACTIVE_WORKSPACE_PREFERENCE, workspace_id],
        )
        .map(|_| ())
        .map_err(WorkspaceError::Storage)
}

fn read_snapshot(connection: &Connection) -> Result<WorkspaceSnapshot, WorkspaceError> {
    let workspaces = read_workspaces(connection)?;
    let notes = connection.prepare("SELECT id, workspace_id, markdown, note_type, created_at, updated_at, pinned FROM notes ORDER BY created_at")
        .map_err(WorkspaceError::Storage)?.query_map([], |row| Ok(Note { id: row.get(0)?, workspace_id: row.get(1)?, markdown: row.get(2)?, note_type: row.get(3)?, created_at: row.get(4)?, updated_at: row.get(5)?, pinned: row.get::<_, i64>(6)? != 0 }))
        .map_err(WorkspaceError::Storage)?.collect::<Result<Vec<_>, _>>().map_err(WorkspaceError::Storage)?;
    let active_workspace_id = read_active_workspace_id(connection)?
        .filter(|id| workspaces.iter().any(|workspace| &workspace.id == id))
        .or_else(|| workspaces.first().map(|workspace| workspace.id.clone()))
        .unwrap_or_default();
    Ok(WorkspaceSnapshot {
        workspaces,
        notes,
        active_workspace_id,
    })
}

fn id() -> String {
    Uuid::new_v4().to_string()
}
fn timestamp() -> String {
    Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Timestamps order lifecycle outcomes, so tests must not collapse them.
    fn distinct_moment() {
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    struct MemoryStore {
        workspaces: Vec<ThinkingWorkspace>,
        notes: Vec<Note>,
        active_workspace_id: String,
    }

    impl MemoryStore {
        fn new() -> Self {
            let mut store = Self {
                workspaces: vec![],
                notes: vec![],
                active_workspace_id: String::new(),
            };
            store.create_workspace(DEFAULT_WORKSPACE_NAME).unwrap();
            store
        }
    }

    impl ThinkingWorkspaceInterface for MemoryStore {
        fn snapshot(&self) -> Result<WorkspaceSnapshot, WorkspaceError> {
            Ok(WorkspaceSnapshot {
                workspaces: self.workspaces.clone(),
                notes: self.notes.clone(),
                active_workspace_id: self.active_workspace_id.clone(),
            })
        }

        fn create_workspace(&mut self, name: &str) -> Result<WorkspaceSnapshot, WorkspaceError> {
            let name = validated_workspace_name(name)?;
            let now = timestamp();
            let workspace = ThinkingWorkspace {
                id: id(),
                name,
                created_at: now.clone(),
                updated_at: now,
            };
            self.active_workspace_id = workspace.id.clone();
            self.workspaces.push(workspace);
            self.snapshot()
        }

        fn select_workspace(
            &mut self,
            workspace_id: &str,
        ) -> Result<WorkspaceSnapshot, WorkspaceError> {
            require_workspace(&self.workspaces, workspace_id)?;
            self.active_workspace_id = workspace_id.to_owned();
            self.snapshot()
        }

        fn rename_workspace(
            &mut self,
            workspace_id: &str,
            name: &str,
        ) -> Result<WorkspaceSnapshot, WorkspaceError> {
            let name = validated_workspace_name(name)?;
            require_workspace(&self.workspaces, workspace_id)?;
            for workspace in self.workspaces.iter_mut() {
                if workspace.id == workspace_id {
                    workspace.name = name.clone();
                    workspace.updated_at = timestamp();
                }
            }
            self.snapshot()
        }

        fn delete_workspace(
            &mut self,
            workspace_id: &str,
        ) -> Result<WorkspaceSnapshot, WorkspaceError> {
            require_workspace(&self.workspaces, workspace_id)?;
            match surviving_workspace_id(&self.workspaces, workspace_id) {
                Some(survivor_id) => {
                    self.workspaces.retain(|workspace| workspace.id != workspace_id);
                    self.notes.retain(|note| note.workspace_id != workspace_id);
                    if self.active_workspace_id == workspace_id {
                        self.active_workspace_id = survivor_id;
                    }
                }
                None => {
                    self.notes.retain(|note| note.workspace_id != workspace_id);
                    for workspace in self.workspaces.iter_mut() {
                        if workspace.id == workspace_id {
                            workspace.name = DEFAULT_WORKSPACE_NAME.to_owned();
                            workspace.updated_at = timestamp();
                        }
                    }
                    self.active_workspace_id = workspace_id.to_owned();
                }
            }
            self.snapshot()
        }

        fn create_note(
            &mut self,
            workspace_id: &str,
            markdown: &str,
        ) -> Result<WorkspaceSnapshot, WorkspaceError> {
            if markdown.trim().is_empty() {
                return Err(WorkspaceError::EmptyNote);
            }
            require_workspace(&self.workspaces, workspace_id)?;
            let now = timestamp();
            self.notes.push(Note {
                id: id(),
                workspace_id: workspace_id.into(),
                markdown: markdown.into(),
                note_type: "general".into(),
                created_at: now.clone(),
                updated_at: now,
                pinned: false,
            });
            self.snapshot()
        }
    }

    fn committed(result: WorkspaceCommandResult) -> WorkspaceSnapshot {
        match result {
            WorkspaceCommandResult::Committed { snapshot } => snapshot,
            other => panic!("intent must commit, got {other:?}"),
        }
    }

    fn workspace_id_named(snapshot: &WorkspaceSnapshot, name: &str) -> String {
        snapshot
            .workspaces
            .iter()
            .find(|workspace| workspace.name == name)
            .unwrap_or_else(|| panic!("no Workspace named {name}"))
            .id
            .clone()
    }

    fn is_validation_failure(result: &WorkspaceCommandResult) -> bool {
        matches!(
            result,
            WorkspaceCommandResult::Failed {
                failure: WorkspaceFailure {
                    code: WorkspaceFailureCode::Validation,
                    ..
                }
            }
        )
    }

    fn is_not_found_failure(result: &WorkspaceCommandResult) -> bool {
        matches!(
            result,
            WorkspaceCommandResult::Failed {
                failure: WorkspaceFailure {
                    code: WorkspaceFailureCode::NotFound,
                    ..
                }
            }
        )
    }

    fn conformance(mut workspace: impl ThinkingWorkspaceInterface) {
        let initial = committed(workspace.snapshot_outcome());
        assert_eq!(initial.workspaces.len(), 1);
        assert_eq!(initial.active_workspace_id, initial.workspaces[0].id);

        // Create selects the new Workspace and keeps duplicate names distinct.
        let created = committed(workspace.create_workspace_outcome("Research"));
        distinct_moment();
        let duplicate = committed(workspace.create_workspace_outcome("  Research  "));
        assert_eq!(duplicate.workspaces.len(), 3);
        let research = workspace_id_named(&created, "Research");
        let duplicates: Vec<_> = duplicate
            .workspaces
            .iter()
            .filter(|candidate| candidate.name == "Research")
            .collect();
        assert_eq!(duplicates.len(), 2);
        assert_ne!(duplicates[0].id, duplicates[1].id);
        assert_eq!(duplicate.active_workspace_id, duplicates[1].id);

        // Selection and rename are explicit intents.
        let selected = committed(workspace.select_workspace_outcome(&research));
        assert_eq!(selected.active_workspace_id, research);
        let long_name = "🧠".repeat(MAX_WORKSPACE_NAME_SCALARS);
        let renamed = committed(workspace.rename_workspace_outcome(&research, &long_name));
        assert_eq!(workspace_id_named(&renamed, &long_name), research);
        assert_eq!(renamed.active_workspace_id, research);

        // Names are required and bounded in Unicode scalar values.
        assert!(is_validation_failure(
            &workspace.create_workspace_outcome("   ")
        ));
        assert!(is_validation_failure(&workspace.create_workspace_outcome(
            &"🧠".repeat(MAX_WORKSPACE_NAME_SCALARS + 1)
        )));
        assert!(is_validation_failure(
            &workspace.rename_workspace_outcome(&research, "\t\n ")
        ));
        assert!(is_not_found_failure(
            &workspace.rename_workspace_outcome("missing", "Nope")
        ));
        assert!(is_not_found_failure(
            &workspace.select_workspace_outcome("missing")
        ));
        assert!(is_not_found_failure(
            &workspace.delete_workspace_outcome("missing")
        ));

        // Notes belong to a Workspace and go with it when it is deleted.
        let with_note = committed(workspace.create_note_outcome(&research, "# A durable thought"));
        assert_eq!(with_note.notes.len(), 1);
        assert_eq!(with_note.notes[0].markdown, "# A durable thought");
        assert_eq!(with_note.notes[0].note_type, "general");
        assert!(is_not_found_failure(
            &workspace.create_note_outcome("missing", "Nope")
        ));

        // Deleting the active Workspace selects the most recently updated survivor.
        distinct_moment();
        let recent = workspace_id_named(
            &committed(workspace.create_workspace_outcome("Most recent")),
            "Most recent",
        );
        committed(workspace.select_workspace_outcome(&research));
        let after_delete = committed(workspace.delete_workspace_outcome(&research));
        assert_eq!(after_delete.active_workspace_id, recent);
        assert_eq!(after_delete.workspaces.len(), 3);
        assert!(after_delete.notes.is_empty());

        // Deleting an inactive Workspace leaves the selection alone.
        let inactive = workspace_id_named(&after_delete, DEFAULT_WORKSPACE_NAME);
        let after_inactive_delete = committed(workspace.delete_workspace_outcome(&inactive));
        assert_eq!(after_inactive_delete.active_workspace_id, recent);

        // Deleting the only Workspace resets it instead of leaving none.
        for remaining in after_inactive_delete
            .workspaces
            .iter()
            .map(|workspace| workspace.id.clone())
        {
            committed(workspace.delete_workspace_outcome(&remaining));
        }
        let last = committed(workspace.snapshot_outcome());
        assert_eq!(last.workspaces.len(), 1);
        assert_eq!(last.workspaces[0].name, DEFAULT_WORKSPACE_NAME);
        assert_eq!(last.active_workspace_id, last.workspaces[0].id);
        assert!(last.notes.is_empty());
    }

    fn temporary_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("nodepad-{}.sqlite", id()))
    }

    fn remove_database(path: &std::path::Path) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

    #[test]
    fn conformance_passes_for_memory_adapter() {
        conformance(MemoryStore::new());
    }

    #[test]
    fn conformance_passes_for_sqlite_adapter() {
        let path = temporary_path();
        conformance(WorkspaceStore::open(&path).unwrap());
        remove_database(&path);
    }

    #[test]
    fn sqlite_recovers_committed_workspace_and_note_after_reopen() {
        let path = temporary_path();
        let (workspace_id, note_id) = {
            let mut store = WorkspaceStore::open(&path).unwrap();
            let snapshot = store.create_workspace("Research").unwrap();
            let workspace_id = snapshot
                .workspaces
                .iter()
                .find(|workspace| workspace.name == "Research")
                .unwrap()
                .id
                .clone();
            let snapshot = store
                .create_note(&workspace_id, "Committed before close")
                .unwrap();
            (workspace_id, snapshot.notes[0].id.clone())
        };
        let reopened = WorkspaceStore::open(&path).unwrap().snapshot().unwrap();
        assert!(reopened
            .workspaces
            .iter()
            .any(|workspace| workspace.id == workspace_id));
        assert!(reopened
            .notes
            .iter()
            .any(|note| note.id == note_id && note.markdown == "Committed before close"));
        remove_database(&path);
    }

    #[test]
    fn selection_and_rename_survive_reopen_and_fall_back_when_the_selection_is_gone() {
        let path = temporary_path();
        let (selected, renamed_name) = {
            let mut store = WorkspaceStore::open(&path).unwrap();
            let snapshot = store.create_workspace("Research").unwrap();
            let research = snapshot
                .workspaces
                .iter()
                .find(|workspace| workspace.name == "Research")
                .unwrap()
                .id
                .clone();
            store.create_workspace("Reading").unwrap();
            store.select_workspace(&research).unwrap();
            store.rename_workspace(&research, " Rêverie ").unwrap();
            (research, "Rêverie".to_owned())
        };

        let reopened = WorkspaceStore::open(&path).unwrap().snapshot().unwrap();
        assert_eq!(reopened.active_workspace_id, selected);
        assert_eq!(
            reopened
                .workspaces
                .iter()
                .find(|workspace| workspace.id == selected)
                .unwrap()
                .name,
            renamed_name
        );

        // A selection that no longer exists falls back to a valid Workspace.
        {
            let store = WorkspaceStore::open(&path).unwrap();
            write_active_workspace_id(&store.connection, "vanished").unwrap();
        }
        let recovered = WorkspaceStore::open(&path).unwrap().snapshot().unwrap();
        assert!(recovered
            .workspaces
            .iter()
            .any(|workspace| workspace.id == recovered.active_workspace_id));
        remove_database(&path);
    }

    #[test]
    fn deleted_workspace_and_its_notes_stay_deleted_after_reopen() {
        let path = temporary_path();
        let survivor = {
            let mut store = WorkspaceStore::open(&path).unwrap();
            let snapshot = store.create_workspace("Research").unwrap();
            let research = snapshot.active_workspace_id.clone();
            store.create_note(&research, "Goes with its Workspace").unwrap();
            let after_delete = store.delete_workspace(&research).unwrap();
            assert!(after_delete.notes.is_empty());
            after_delete.active_workspace_id
        };
        let reopened = WorkspaceStore::open(&path).unwrap().snapshot().unwrap();
        assert_eq!(reopened.workspaces.len(), 1);
        assert_eq!(reopened.active_workspace_id, survivor);
        assert!(reopened.notes.is_empty());
        remove_database(&path);
    }

    #[test]
    fn storage_failure_returns_no_success_or_partial_workspace_or_note() {
        let path = temporary_path();
        let mut store = WorkspaceStore::open(&path).unwrap();
        let before = store.snapshot().unwrap();
        store.connection.execute_batch("CREATE TRIGGER reject_workspaces BEFORE INSERT ON thinking_workspaces BEGIN SELECT RAISE(FAIL, 'injected'); END;").unwrap();
        assert!(matches!(
            store.create_workspace_outcome("Must not persist"),
            WorkspaceCommandResult::Failed { .. }
        ));
        assert_eq!(store.snapshot().unwrap(), before);
        store
            .connection
            .execute_batch("DROP TRIGGER reject_workspaces;")
            .unwrap();

        let workspace = store.create_workspace("Research").unwrap().active_workspace_id;
        store.connection.execute_batch("CREATE TRIGGER reject_notes BEFORE INSERT ON notes BEGIN SELECT RAISE(FAIL, 'injected'); END;").unwrap();
        assert!(matches!(
            store.create_note_outcome(&workspace, "Must not persist"),
            WorkspaceCommandResult::Failed { .. }
        ));
        assert!(store.snapshot().unwrap().notes.is_empty());
        store
            .connection
            .execute_batch("DROP TRIGGER reject_notes;")
            .unwrap();

        // A rejected delete leaves the Workspace, its Notes, and the selection intact.
        store.create_note(&workspace, "Survives a failed delete").unwrap();
        let before_delete = store.snapshot().unwrap();
        store.connection.execute_batch("CREATE TRIGGER reject_deletes BEFORE DELETE ON thinking_workspaces BEGIN SELECT RAISE(FAIL, 'injected'); END;").unwrap();
        assert!(matches!(
            store.delete_workspace_outcome(&workspace),
            WorkspaceCommandResult::Failed { .. }
        ));
        assert_eq!(store.snapshot().unwrap(), before_delete);
        drop(store);
        remove_database(&path);
    }

    #[test]
    fn an_unreadable_database_reports_a_category_and_creates_nothing() {
        let directory = std::env::temp_dir().join(format!("nodepad-open-{}", id()));
        std::fs::create_dir_all(&directory).unwrap();
        // A directory can never be a database file, so the open must fail.
        let failure = WorkspaceStore::open(&directory).unwrap_err();
        assert_eq!(failure.category, StorageOpenFailureCategory::Unreadable);
        assert!(std::fs::read_dir(&directory).unwrap().next().is_none());
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn a_migration_failure_reports_a_category_and_never_resets_existing_data() {
        let path = temporary_path();
        let existing = {
            let mut store = WorkspaceStore::open(&path).unwrap();
            store.create_workspace("Precious").unwrap()
        };

        // Force the next open to re-run a migration and fail while recording it.
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute("DELETE FROM schema_migrations WHERE version = 2", [])
                .unwrap();
            connection
                .execute_batch("CREATE TRIGGER reject_migrations BEFORE INSERT ON schema_migrations BEGIN SELECT RAISE(FAIL, 'injected'); END;")
                .unwrap();
        }
        let failure = WorkspaceStore::open(&path).unwrap_err();
        assert_eq!(failure.category, StorageOpenFailureCategory::Migration);

        // Nothing was reset: the Workspaces and Notes are still on disk.
        let connection = Connection::open(&path).unwrap();
        let names: Vec<String> = connection
            .prepare("SELECT name FROM thinking_workspaces ORDER BY created_at")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            names,
            existing
                .workspaces
                .iter()
                .map(|workspace| workspace.name.clone())
                .collect::<Vec<_>>()
        );
        drop(connection);
        remove_database(&path);
    }
}
