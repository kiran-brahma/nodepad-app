use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

const DEFAULT_WORKSPACE_NAME: &str = "My Thinking Workspace";

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

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum WorkspaceCommandResult {
    Committed { snapshot: WorkspaceSnapshot },
    Failed { failure: WorkspaceFailure },
}

#[derive(Debug, Error)]
pub(crate) enum WorkspaceError {
    #[error("A Thinking Workspace name is required.")]
    EmptyWorkspaceName,
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
            Self::EmptyWorkspaceName | Self::EmptyNote => WorkspaceFailureCode::Validation,
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
    fn create_note_outcome(
        &mut self,
        workspace_id: &str,
        markdown: &str,
    ) -> WorkspaceCommandResult {
        outcome(self.create_note(workspace_id, markdown))
    }
}

pub struct WorkspaceStore {
    connection: Connection,
}

impl WorkspaceStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, WorkspaceError> {
        let mut connection = Connection::open(path).map_err(WorkspaceError::Storage)?;
        migrate(&mut connection)?;
        let mut store = Self { connection };
        store.ensure_default_workspace()?;
        Ok(store)
    }

    fn ensure_default_workspace(&mut self) -> Result<(), WorkspaceError> {
        let existing: Option<String> = self
            .connection
            .query_row("SELECT id FROM thinking_workspaces LIMIT 1", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(WorkspaceError::Storage)?;
        if existing.is_none() {
            self.create_workspace(DEFAULT_WORKSPACE_NAME)?;
        }
        Ok(())
    }
}

impl ThinkingWorkspaceInterface for WorkspaceStore {
    fn snapshot(&self) -> Result<WorkspaceSnapshot, WorkspaceError> {
        read_snapshot(&self.connection)
    }

    fn create_workspace(&mut self, name: &str) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(WorkspaceError::EmptyWorkspaceName);
        }
        let mut snapshot = self.snapshot()?;
        let now = timestamp();
        let workspace = ThinkingWorkspace {
            id: id(),
            name: name.into(),
            created_at: now.clone(),
            updated_at: now,
        };
        let transaction = self
            .connection
            .transaction()
            .map_err(WorkspaceError::Storage)?;
        transaction.execute(
            "INSERT INTO thinking_workspaces (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            params![workspace.id, workspace.name, workspace.created_at],
        ).map_err(WorkspaceError::Storage)?;
        transaction.commit().map_err(WorkspaceError::Storage)?;
        snapshot.workspaces.push(workspace);
        Ok(snapshot)
    }

    fn create_note(
        &mut self,
        workspace_id: &str,
        markdown: &str,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        if markdown.trim().is_empty() {
            return Err(WorkspaceError::EmptyNote);
        }
        let mut snapshot = self.snapshot()?;
        if !snapshot
            .workspaces
            .iter()
            .any(|workspace| workspace.id == workspace_id)
        {
            return Err(WorkspaceError::WorkspaceNotFound);
        }
        let now = timestamp();
        let note = Note {
            id: id(),
            workspace_id: workspace_id.into(),
            markdown: markdown.into(),
            note_type: "general".into(),
            created_at: now.clone(),
            updated_at: now,
            pinned: false,
        };
        let transaction = self
            .connection
            .transaction()
            .map_err(WorkspaceError::Storage)?;
        transaction.execute(
            "INSERT INTO notes (id, workspace_id, markdown, note_type, created_at, updated_at, pinned) VALUES (?1, ?2, ?3, 'general', ?4, ?4, 0)",
            params![note.id, note.workspace_id, note.markdown, note.created_at],
        ).map_err(WorkspaceError::Storage)?;
        transaction.commit().map_err(WorkspaceError::Storage)?;
        snapshot.notes.push(note);
        Ok(snapshot)
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

fn migrate(connection: &mut Connection) -> Result<(), WorkspaceError> {
    connection
        .execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")
        .map_err(WorkspaceError::Storage)?;
    let migrations = [(1_i64, include_str!("../migrations/0001_initial.sql"))];
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

fn read_snapshot(connection: &Connection) -> Result<WorkspaceSnapshot, WorkspaceError> {
    let workspaces = connection
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
        .map_err(WorkspaceError::Storage)?;
    let notes = connection.prepare("SELECT id, workspace_id, markdown, note_type, created_at, updated_at, pinned FROM notes ORDER BY created_at")
        .map_err(WorkspaceError::Storage)?.query_map([], |row| Ok(Note { id: row.get(0)?, workspace_id: row.get(1)?, markdown: row.get(2)?, note_type: row.get(3)?, created_at: row.get(4)?, updated_at: row.get(5)?, pinned: row.get::<_, i64>(6)? != 0 }))
        .map_err(WorkspaceError::Storage)?.collect::<Result<Vec<_>, _>>().map_err(WorkspaceError::Storage)?;
    Ok(WorkspaceSnapshot { workspaces, notes })
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

    struct MemoryStore {
        snapshot: WorkspaceSnapshot,
    }
    impl MemoryStore {
        fn new() -> Self {
            let mut store = Self {
                snapshot: WorkspaceSnapshot {
                    workspaces: vec![],
                    notes: vec![],
                },
            };
            store.create_workspace(DEFAULT_WORKSPACE_NAME).unwrap();
            store
        }
    }
    impl ThinkingWorkspaceInterface for MemoryStore {
        fn snapshot(&self) -> Result<WorkspaceSnapshot, WorkspaceError> {
            Ok(self.snapshot.clone())
        }
        fn create_workspace(&mut self, name: &str) -> Result<WorkspaceSnapshot, WorkspaceError> {
            let name = name.trim();
            if name.is_empty() {
                return Err(WorkspaceError::EmptyWorkspaceName);
            }
            let now = timestamp();
            self.snapshot.workspaces.push(ThinkingWorkspace {
                id: id(),
                name: name.into(),
                created_at: now.clone(),
                updated_at: now,
            });
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
            if !self
                .snapshot
                .workspaces
                .iter()
                .any(|workspace| workspace.id == workspace_id)
            {
                return Err(WorkspaceError::WorkspaceNotFound);
            }
            let now = timestamp();
            self.snapshot.notes.push(Note {
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

    fn conformance(mut workspace: impl ThinkingWorkspaceInterface) {
        let WorkspaceCommandResult::Committed { snapshot: initial } = workspace.snapshot_outcome()
        else {
            panic!("default Workspace must be committed")
        };
        assert_eq!(initial.workspaces.len(), 1);
        let WorkspaceCommandResult::Committed {
            snapshot: workspaces,
        } = workspace.create_workspace_outcome("Research")
        else {
            panic!("Workspace must commit")
        };
        let workspace_id = workspaces
            .workspaces
            .iter()
            .find(|workspace| workspace.name == "Research")
            .unwrap()
            .id
            .clone();
        let WorkspaceCommandResult::Committed { snapshot } =
            workspace.create_note_outcome(&workspace_id, "# A durable thought")
        else {
            panic!("Note must commit")
        };
        assert_eq!(snapshot.workspaces.len(), 2);
        assert_eq!(snapshot.notes[0].markdown, "# A durable thought");
        assert_eq!(snapshot.notes[0].note_type, "general");
        assert!(matches!(
            workspace.create_note_outcome("missing", "Nope"),
            WorkspaceCommandResult::Failed {
                failure: WorkspaceFailure {
                    code: WorkspaceFailureCode::NotFound,
                    ..
                }
            }
        ));
    }

    #[test]
    fn conformance_passes_for_memory_adapter() {
        conformance(MemoryStore::new());
    }

    #[test]
    fn conformance_passes_for_sqlite_adapter() {
        let path = std::env::temp_dir().join(format!("nodepad-{}.sqlite", id()));
        conformance(WorkspaceStore::open(&path).unwrap());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn sqlite_recovers_committed_workspace_and_note_after_reopen() {
        let path = std::env::temp_dir().join(format!("nodepad-{}.sqlite", id()));
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
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn storage_failure_returns_no_success_or_partial_workspace_or_note() {
        let path = std::env::temp_dir().join(format!("nodepad-{}.sqlite", id()));
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
        let workspace = store
            .create_workspace("Research")
            .unwrap()
            .workspaces
            .iter()
            .find(|workspace| workspace.name == "Research")
            .unwrap()
            .id
            .clone();
        store.connection.execute_batch("CREATE TRIGGER reject_notes BEFORE INSERT ON notes BEGIN SELECT RAISE(FAIL, 'injected'); END;").unwrap();
        assert!(matches!(
            store.create_note_outcome(&workspace, "Must not persist"),
            WorkspaceCommandResult::Failed { .. }
        ));
        assert!(store.snapshot().unwrap().notes.is_empty());
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
}
