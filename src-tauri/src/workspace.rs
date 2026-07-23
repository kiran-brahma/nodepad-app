use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

const DEFAULT_WORKSPACE_NAME: &str = "My Thinking Workspace";
/// Names are bounded in Unicode scalar values, never bytes or grapheme clusters.
const MAX_WORKSPACE_NAME_SCALARS: usize = 120;
/// Annotations are bounded in Unicode scalar values for the same reason.
const MAX_ANNOTATION_SCALARS: usize = 2_000;
const MAX_LABEL_NAME_SCALARS: usize = 60;
const ACTIVE_WORKSPACE_PREFERENCE: &str = "active_workspace_id";
const DEFAULT_NOTE_TYPE: &str = "general";
/// The fixed structural classifications a Note may carry. Nothing outside this
/// set is durable, in the schema or in the interface.
const NOTE_TYPES: [&str; 14] = [
    "claim",
    "question",
    "task",
    "idea",
    "entity",
    "quote",
    "reference",
    "definition",
    "opinion",
    "reflection",
    "narrative",
    "comparison",
    "thesis",
    "general",
];
/// Undo is a bounded in-memory convenience, never a durable log.
const MAX_REVERSIBLE_COMMANDS: usize = 20;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingWorkspace {
    id: String,
    name: String,
    created_at: String,
    updated_at: String,
}

/// Who last decided a value. Manual authorship is durable so a later AI slice
/// cannot overwrite a thinker's choice silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    Default,
    Manual,
}

impl Provenance {
    fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Manual => "manual",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "manual" => Self::Manual,
            _ => Self::Default,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    id: String,
    workspace_id: String,
    markdown: String,
    note_type: String,
    note_type_provenance: Provenance,
    annotation: Option<String>,
    annotation_provenance: Provenance,
    created_at: String,
    updated_at: String,
    pinned: bool,
    labels: Vec<Label>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    id: String,
    workspace_id: String,
    name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    note_id: String,
    snippet: String,
    note_type: String,
    labels: Vec<Label>,
    rank: f64,
}

/// The only shapes a Note row may change in. Every Note intent and every undo
/// is one of these, so an undo is an ordinary committed mutation rather than a
/// rewind of the database file.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum NoteMutation {
    Insert(Note),
    Replace(Note),
    Delete { note_id: String },
}

impl NoteMutation {
    fn workspace_id(&self) -> &str {
        match self {
            Self::Insert(note) | Self::Replace(note) => &note.workspace_id,
            Self::Delete { .. } => "",
        }
    }
}

/// Bounded per-Workspace history of compensating mutations for this session.
/// It is deliberately in memory: restart clears undo without touching durable
/// state.
#[derive(Debug, Default)]
pub(crate) struct UndoHistory {
    commands: Vec<(String, NoteMutation)>,
}

impl UndoHistory {
    fn push(&mut self, workspace_id: &str, compensation: NoteMutation) {
        self.commands
            .push((workspace_id.to_owned(), compensation));
        // The bound is per Workspace, so only this Workspace's oldest command is
        // dropped when it overflows.
        let mut kept = 0;
        let mut retain: Vec<bool> = vec![true; self.commands.len()];
        for index in (0..self.commands.len()).rev() {
            if self.commands[index].0 == workspace_id {
                kept += 1;
                retain[index] = kept <= MAX_REVERSIBLE_COMMANDS;
            }
        }
        let mut index = 0;
        self.commands.retain(|_| {
            let keep = retain[index];
            index += 1;
            keep
        });
    }

    fn take_last(&mut self, workspace_id: &str) -> Option<NoteMutation> {
        let position = self
            .commands
            .iter()
            .rposition(|(id, _)| id == workspace_id)?;
        Some(self.commands.remove(position).1)
    }

    /// Returns a taken command to its place after a failed undo, so a storage
    /// failure never silently costs the thinker a reversible step.
    fn restore(&mut self, workspace_id: &str, compensation: NoteMutation) {
        self.commands
            .push((workspace_id.to_owned(), compensation));
    }

    fn clear(&mut self, workspace_id: &str) {
        self.commands.retain(|(id, _)| id != workspace_id);
    }

    fn depth(&self, workspace_id: &str) -> usize {
        self.commands
            .iter()
            .filter(|(id, _)| id == workspace_id)
            .count()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    workspaces: Vec<ThinkingWorkspace>,
    notes: Vec<Note>,
    active_workspace_id: String,
    /// How many mutations in the active Workspace can still be undone in this
    /// session. Always zero right after a restart.
    undoable_commands: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceFailureCode {
    Validation,
    NotFound,
    NothingToUndo,
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

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum WorkspaceSearchOutcome {
    Committed { results: Vec<SearchResult> },
    Failed { failure: WorkspaceFailure },
}

pub fn unavailable_search_outcome(failure: &StorageOpenFailure) -> WorkspaceSearchOutcome {
    WorkspaceSearchOutcome::Failed {
        failure: WorkspaceFailure {
            code: WorkspaceFailureCode::Storage,
            message: failure.message.clone(),
        },
    }
}

#[derive(Debug, Error)]
pub(crate) enum WorkspaceError {
    #[error("A Thinking Workspace name is required.")]
    EmptyWorkspaceName,
    #[error("A Thinking Workspace name may not exceed {MAX_WORKSPACE_NAME_SCALARS} characters.")]
    WorkspaceNameTooLong,
    #[error("A Note needs Markdown text.")]
    EmptyNote,
    #[error("An Annotation may not exceed {MAX_ANNOTATION_SCALARS} characters.")]
    AnnotationTooLong,
    #[error("That is not a Note Type Nodepad recognizes.")]
    UnknownNoteType,
    #[error("A Label needs one to four words and may not exceed {MAX_LABEL_NAME_SCALARS} characters.")]
    InvalidLabelName,
    #[error("That Label no longer exists.")]
    LabelNotFound,
    #[error("The selected Thinking Workspace no longer exists.")]
    WorkspaceNotFound,
    #[error("That Note no longer exists.")]
    NoteNotFound,
    #[error("There is nothing left to undo in this Thinking Workspace.")]
    NothingToUndo,
    #[error("Local storage could not commit this change. Please try again.")]
    Storage(#[source] rusqlite::Error),
}

impl WorkspaceError {
    fn failure(&self) -> WorkspaceFailure {
        let code = match self {
            Self::EmptyWorkspaceName
            | Self::WorkspaceNameTooLong
            | Self::EmptyNote
            | Self::AnnotationTooLong
            | Self::UnknownNoteType | Self::InvalidLabelName => WorkspaceFailureCode::Validation,
            Self::WorkspaceNotFound | Self::NoteNotFound | Self::LabelNotFound => WorkspaceFailureCode::NotFound,
            Self::NothingToUndo => WorkspaceFailureCode::NothingToUndo,
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
    /// Removes a Workspace without touching undo history; callers use
    /// `delete_workspace`, which also drops the now-meaningless history.
    fn remove_workspace(&mut self, workspace_id: &str)
        -> Result<WorkspaceSnapshot, WorkspaceError>;
    /// Commits exactly one Note row change, atomically.
    fn apply_note_mutation(&mut self, mutation: &NoteMutation) -> Result<(), WorkspaceError>;
    fn history(&mut self) -> &mut UndoHistory;
    fn attach_label(&mut self, note_id: &str, name: &str) -> Result<WorkspaceSnapshot, WorkspaceError>;
    fn detach_label(&mut self, note_id: &str, label_id: &str) -> Result<WorkspaceSnapshot, WorkspaceError>;
    fn rename_label(&mut self, label_id: &str, name: &str) -> Result<WorkspaceSnapshot, WorkspaceError>;
    fn remove_label(&mut self, label_id: &str) -> Result<WorkspaceSnapshot, WorkspaceError>;
    fn search_notes(&self, workspace_id: &str, query: &str) -> Result<Vec<SearchResult>, WorkspaceError>;

    fn delete_workspace(
        &mut self,
        workspace_id: &str,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let snapshot = self.remove_workspace(workspace_id)?;
        // Its Notes are gone, so every compensation naming them is void.
        self.history().clear(workspace_id);
        self.snapshot().or(Ok(snapshot))
    }

    fn note(&self, note_id: &str) -> Result<Note, WorkspaceError> {
        self.snapshot()?
            .notes
            .into_iter()
            .find(|note| note.id == note_id)
            .ok_or(WorkspaceError::NoteNotFound)
    }

    fn create_note(
        &mut self,
        workspace_id: &str,
        markdown: &str,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let markdown = validated_markdown(markdown)?;
        require_workspace(&self.snapshot()?.workspaces, workspace_id)?;
        let now = timestamp();
        let note = Note {
            id: id(),
            workspace_id: workspace_id.to_owned(),
            markdown,
            note_type: DEFAULT_NOTE_TYPE.to_owned(),
            note_type_provenance: Provenance::Default,
            annotation: None,
            annotation_provenance: Provenance::Default,
            created_at: now.clone(),
            updated_at: now,
            pinned: false,
            labels: vec![],
        };
        let note_id = note.id.clone();
        self.commit_note(NoteMutation::Insert(note), NoteMutation::Delete { note_id })
    }

    fn edit_note_text(
        &mut self,
        note_id: &str,
        markdown: &str,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let markdown = validated_markdown(markdown)?;
        let previous = self.note(note_id)?;
        let mut edited = previous.clone();
        edited.markdown = markdown;
        edited.updated_at = timestamp();
        self.commit_note(
            NoteMutation::Replace(edited),
            NoteMutation::Replace(previous),
        )
    }

    fn set_note_type(
        &mut self,
        note_id: &str,
        note_type: &str,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let note_type = validated_note_type(note_type)?;
        let previous = self.note(note_id)?;
        let mut typed = previous.clone();
        typed.note_type = note_type;
        typed.note_type_provenance = Provenance::Manual;
        typed.updated_at = timestamp();
        self.commit_note(NoteMutation::Replace(typed), NoteMutation::Replace(previous))
    }

    /// Blank Annotation text clears the Annotation; both are manual authorship.
    fn set_note_annotation(
        &mut self,
        note_id: &str,
        annotation: &str,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let annotation = validated_annotation(annotation)?;
        let previous = self.note(note_id)?;
        let mut annotated = previous.clone();
        annotated.annotation = annotation;
        annotated.annotation_provenance = Provenance::Manual;
        annotated.updated_at = timestamp();
        self.commit_note(
            NoteMutation::Replace(annotated),
            NoteMutation::Replace(previous),
        )
    }

    fn set_note_pinned(
        &mut self,
        note_id: &str,
        pinned: bool,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let previous = self.note(note_id)?;
        let mut repinned = previous.clone();
        repinned.pinned = pinned;
        repinned.updated_at = timestamp();
        self.commit_note(
            NoteMutation::Replace(repinned),
            NoteMutation::Replace(previous),
        )
    }

    fn delete_note(&mut self, note_id: &str) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let previous = self.note(note_id)?;
        self.commit_note(
            NoteMutation::Delete {
                note_id: note_id.to_owned(),
            },
            // Restoring the whole row keeps the Note's identity and fields.
            NoteMutation::Insert(previous),
        )
    }

    /// Undo commits a new compensating transaction; it never rewinds storage.
    fn undo(&mut self, workspace_id: &str) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let compensation = self
            .history()
            .take_last(workspace_id)
            .ok_or(WorkspaceError::NothingToUndo)?;
        match self.apply_note_mutation(&compensation) {
            Ok(()) => self.snapshot(),
            Err(error) => {
                self.history().restore(workspace_id, compensation);
                Err(error)
            }
        }
    }

    /// Commits an intent and records its compensation only after the commit, so
    /// a failed mutation leaves neither durable state nor history changed.
    fn commit_note(
        &mut self,
        mutation: NoteMutation,
        compensation: NoteMutation,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let workspace_id = match mutation.workspace_id() {
            "" => compensation.workspace_id().to_owned(),
            id => id.to_owned(),
        };
        self.apply_note_mutation(&mutation)?;
        self.history().push(&workspace_id, compensation);
        self.snapshot()
    }

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
    fn edit_note_text_outcome(&mut self, note_id: &str, markdown: &str) -> WorkspaceCommandResult {
        outcome(self.edit_note_text(note_id, markdown))
    }
    fn set_note_type_outcome(&mut self, note_id: &str, note_type: &str) -> WorkspaceCommandResult {
        outcome(self.set_note_type(note_id, note_type))
    }
    fn set_note_annotation_outcome(
        &mut self,
        note_id: &str,
        annotation: &str,
    ) -> WorkspaceCommandResult {
        outcome(self.set_note_annotation(note_id, annotation))
    }
    fn set_note_pinned_outcome(&mut self, note_id: &str, pinned: bool) -> WorkspaceCommandResult {
        outcome(self.set_note_pinned(note_id, pinned))
    }
    fn delete_note_outcome(&mut self, note_id: &str) -> WorkspaceCommandResult {
        outcome(self.delete_note(note_id))
    }
    fn undo_outcome(&mut self, workspace_id: &str) -> WorkspaceCommandResult {
        outcome(self.undo(workspace_id))
    }
    fn attach_label_outcome(&mut self, note_id: &str, name: &str) -> WorkspaceCommandResult {
        outcome(self.attach_label(note_id, name))
    }
    fn detach_label_outcome(&mut self, note_id: &str, label_id: &str) -> WorkspaceCommandResult {
        outcome(self.detach_label(note_id, label_id))
    }
    fn rename_label_outcome(&mut self, label_id: &str, name: &str) -> WorkspaceCommandResult {
        outcome(self.rename_label(label_id, name))
    }
    fn remove_label_outcome(&mut self, label_id: &str) -> WorkspaceCommandResult {
        outcome(self.remove_label(label_id))
    }
    fn search_outcome(&self, workspace_id: &str, query: &str) -> WorkspaceSearchOutcome {
        match self.search_notes(workspace_id, query) {
            Ok(results) => WorkspaceSearchOutcome::Committed { results },
            Err(error) => WorkspaceSearchOutcome::Failed { failure: error.failure() },
        }
    }
}

#[derive(Debug)]
pub struct WorkspaceStore {
    connection: Connection,
    history: UndoHistory,
}

impl WorkspaceStore {
    /// Opens durable storage. A failure never resets, deletes, or overwrites an
    /// existing database; it reports the category so recovery can retry or quit.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageOpenFailure> {
        Self::open_prepared(path, migrate)
    }

    /// Opening is split from preparing the schema so a failing preparation can
    /// be injected. A failed open on a path that held no database leaves none
    /// behind, so the next launch can never mistake a stub for a fresh start.
    fn open_prepared(
        path: impl AsRef<Path>,
        prepare: fn(&mut Connection) -> Result<(), WorkspaceError>,
    ) -> Result<Self, StorageOpenFailure> {
        let path = path.as_ref();
        let existed = path.exists();
        let opened = Self::open_at(path, prepare);
        if opened.is_err() && !existed {
            discard_database_files(path);
        }
        opened
    }

    fn open_at(
        path: &Path,
        prepare: fn(&mut Connection) -> Result<(), WorkspaceError>,
    ) -> Result<Self, StorageOpenFailure> {
        let mut connection = Connection::open(path).map_err(|error| {
            StorageOpenFailure::from_error(
                StorageOpenFailureCategory::Unreadable,
                WorkspaceError::Storage(error),
            )
        })?;
        prepare(&mut connection).map_err(|error| {
            StorageOpenFailure::from_error(StorageOpenFailureCategory::Migration, error)
        })?;
        // A fresh process starts with no undo history, which is exactly what a
        // restart must leave behind.
        let mut store = Self {
            connection,
            history: UndoHistory::default(),
        };
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
        let mut snapshot = read_snapshot(&self.connection)?;
        snapshot.undoable_commands = self.history.depth(&snapshot.active_workspace_id);
        Ok(snapshot)
    }

    fn history(&mut self) -> &mut UndoHistory {
        &mut self.history
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

    fn remove_workspace(
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
        transaction
            .execute("DELETE FROM note_search WHERE workspace_id = ?1", [workspace_id])
            .map_err(WorkspaceError::Storage)?;
        write_active_workspace_id(&transaction, &next_active)?;
        transaction.commit().map_err(WorkspaceError::Storage)?;
        self.snapshot()
    }

    fn apply_note_mutation(&mut self, mutation: &NoteMutation) -> Result<(), WorkspaceError> {
        let affected_workspace_id = match mutation {
            NoteMutation::Insert(note) | NoteMutation::Replace(note) => Some(note.workspace_id.clone()),
            NoteMutation::Delete { note_id } => self.connection.query_row(
                "SELECT workspace_id FROM notes WHERE id = ?1", [note_id], |row| row.get(0)
            ).optional().map_err(WorkspaceError::Storage)?,
        };
        let transaction = self
            .connection
            .transaction()
            .map_err(WorkspaceError::Storage)?;
        let changed = match mutation {
            NoteMutation::Insert(note) => transaction.execute(
                "INSERT INTO notes (id, workspace_id, markdown, note_type, note_type_provenance, annotation, annotation_provenance, created_at, updated_at, pinned) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    note.id,
                    note.workspace_id,
                    note.markdown,
                    note.note_type,
                    note.note_type_provenance.as_str(),
                    note.annotation,
                    note.annotation_provenance.as_str(),
                    note.created_at,
                    note.updated_at,
                    i64::from(note.pinned),
                ],
            ),
            NoteMutation::Replace(note) => transaction.execute(
                "UPDATE notes SET markdown = ?2, note_type = ?3, note_type_provenance = ?4, annotation = ?5, annotation_provenance = ?6, updated_at = ?7, pinned = ?8 WHERE id = ?1",
                params![
                    note.id,
                    note.markdown,
                    note.note_type,
                    note.note_type_provenance.as_str(),
                    note.annotation,
                    note.annotation_provenance.as_str(),
                    note.updated_at,
                    i64::from(note.pinned),
                ],
            ),
            NoteMutation::Delete { note_id } => {
                transaction.execute("DELETE FROM notes WHERE id = ?1", params![note_id])
            }
        }
        .map_err(WorkspaceError::Storage)?;
        if changed == 0 {
            // The transaction is dropped unfinished, so nothing is committed.
            return Err(WorkspaceError::NoteNotFound);
        }
        if let Some(workspace_id) = affected_workspace_id {
            refresh_workspace_search(&transaction, &workspace_id)?;
        }
        transaction.commit().map_err(WorkspaceError::Storage)
    }

    fn attach_label(&mut self, note_id: &str, name: &str) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let note = self.note(note_id)?;
        let (name, canonical_name) = validated_label_name(name)?;
        let transaction = self.connection.transaction().map_err(WorkspaceError::Storage)?;
        let label_id: Option<String> = transaction.query_row(
            "SELECT id FROM labels WHERE workspace_id = ?1 AND canonical_name = ?2",
            params![note.workspace_id, canonical_name], |row| row.get(0)
        ).optional().map_err(WorkspaceError::Storage)?;
        let label_id = match label_id {
            Some(id) => id,
            None => {
                let id = id();
                transaction.execute("INSERT INTO labels (id, workspace_id, name, canonical_name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)", params![id, note.workspace_id, name, canonical_name, timestamp()]).map_err(WorkspaceError::Storage)?;
                id
            }
        };
        transaction.execute("INSERT OR IGNORE INTO note_labels (note_id, label_id) VALUES (?1, ?2)", params![note_id, label_id]).map_err(WorkspaceError::Storage)?;
        refresh_workspace_search(&transaction, &note.workspace_id)?;
        transaction.commit().map_err(WorkspaceError::Storage)?;
        self.snapshot()
    }

    fn detach_label(&mut self, note_id: &str, label_id: &str) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let note = self.note(note_id)?;
        let transaction = self.connection.transaction().map_err(WorkspaceError::Storage)?;
        transaction.execute("DELETE FROM note_labels WHERE note_id = ?1 AND label_id = ?2", params![note_id, label_id]).map_err(WorkspaceError::Storage)?;
        transaction.execute("DELETE FROM labels WHERE id = ?1 AND NOT EXISTS (SELECT 1 FROM note_labels WHERE label_id = ?1)", [label_id]).map_err(WorkspaceError::Storage)?;
        refresh_workspace_search(&transaction, &note.workspace_id)?;
        transaction.commit().map_err(WorkspaceError::Storage)?;
        self.snapshot()
    }

    fn rename_label(&mut self, label_id: &str, name: &str) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let (name, canonical_name) = validated_label_name(name)?;
        let transaction = self.connection.transaction().map_err(WorkspaceError::Storage)?;
        let (workspace_id, old_id): (String, String) = transaction.query_row("SELECT workspace_id, id FROM labels WHERE id = ?1", [label_id], |row| Ok((row.get(0)?, row.get(1)?))).optional().map_err(WorkspaceError::Storage)?.ok_or(WorkspaceError::LabelNotFound)?;
        let collision: Option<String> = transaction.query_row("SELECT id FROM labels WHERE workspace_id = ?1 AND canonical_name = ?2", params![workspace_id, canonical_name], |row| row.get(0)).optional().map_err(WorkspaceError::Storage)?;
        if let Some(survivor) = collision.filter(|candidate| candidate != &old_id) {
            transaction.execute("INSERT OR IGNORE INTO note_labels (note_id, label_id) SELECT note_id, ?1 FROM note_labels WHERE label_id = ?2", params![survivor, old_id]).map_err(WorkspaceError::Storage)?;
            transaction.execute("DELETE FROM labels WHERE id = ?1", [old_id]).map_err(WorkspaceError::Storage)?;
        } else {
            transaction.execute("UPDATE labels SET name = ?2, canonical_name = ?3, updated_at = ?4 WHERE id = ?1", params![old_id, name, canonical_name, timestamp()]).map_err(WorkspaceError::Storage)?;
        }
        refresh_workspace_search(&transaction, &workspace_id)?;
        transaction.commit().map_err(WorkspaceError::Storage)?;
        self.snapshot()
    }

    fn remove_label(&mut self, label_id: &str) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let transaction = self.connection.transaction().map_err(WorkspaceError::Storage)?;
        let workspace_id: String = transaction.query_row("SELECT workspace_id FROM labels WHERE id = ?1", [label_id], |row| row.get(0)).optional().map_err(WorkspaceError::Storage)?.ok_or(WorkspaceError::LabelNotFound)?;
        transaction.execute("DELETE FROM labels WHERE id = ?1", [label_id]).map_err(WorkspaceError::Storage)?;
        refresh_workspace_search(&transaction, &workspace_id)?;
        transaction.commit().map_err(WorkspaceError::Storage)?;
        self.snapshot()
    }

    fn search_notes(&self, workspace_id: &str, query: &str) -> Result<Vec<SearchResult>, WorkspaceError> {
        require_workspace(&read_workspaces(&self.connection)?, workspace_id)?;
        let query = fts_query(query);
        if query.is_empty() { return Ok(vec![]); }
        let mut statement = self.connection.prepare("SELECT note_id, snippet(note_search, 2, '', '', '…', 24), bm25(note_search) FROM note_search WHERE note_search MATCH ?1 AND workspace_id = ?2 ORDER BY bm25(note_search), note_id").map_err(WorkspaceError::Storage)?;
        let results = statement.query_map(params![query, workspace_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, f64>(2)?))).map_err(WorkspaceError::Storage)?.map(|row| {
            let (note_id, snippet, rank) = row.map_err(WorkspaceError::Storage)?;
            let note_type: String = self.connection.query_row("SELECT note_type FROM notes WHERE id = ?1", [&note_id], |row| row.get(0)).map_err(WorkspaceError::Storage)?;
            Ok(SearchResult { labels: labels_for_note(&self.connection, &note_id)?, note_id, snippet, note_type, rank })
        }).collect();
        results
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

/// Note text is required after trimming but stored exactly as authored, so the
/// thinker's Markdown layout survives a round trip.
fn validated_markdown(markdown: &str) -> Result<String, WorkspaceError> {
    if markdown.trim().is_empty() {
        return Err(WorkspaceError::EmptyNote);
    }
    Ok(markdown.to_owned())
}

fn validated_note_type(note_type: &str) -> Result<String, WorkspaceError> {
    NOTE_TYPES
        .contains(&note_type)
        .then(|| note_type.to_owned())
        .ok_or(WorkspaceError::UnknownNoteType)
}

/// Annotation is plain commentary: trimmed, bounded, and blank means cleared.
fn validated_annotation(annotation: &str) -> Result<Option<String>, WorkspaceError> {
    let annotation = annotation.trim();
    if annotation.is_empty() {
        return Ok(None);
    }
    if annotation.chars().count() > MAX_ANNOTATION_SCALARS {
        return Err(WorkspaceError::AnnotationTooLong);
    }
    Ok(Some(annotation.to_owned()))
}

/// Label identity is Unicode lowercase rather than SQLite NOCASE, whose
/// comparison is ASCII-only. Display spelling remains the thinker's choice.
fn validated_label_name(name: &str) -> Result<(String, String), WorkspaceError> {
    let name = name.trim();
    let words = name.split_whitespace().count();
    if name.is_empty() || words == 0 || words > 4 || name.chars().count() > MAX_LABEL_NAME_SCALARS {
        return Err(WorkspaceError::InvalidLabelName);
    }
    Ok((name.to_owned(), name.to_lowercase()))
}

/// FTS receives quoted words only, never raw query syntax. Punctuation is a
/// separator, so copied search text cannot inject operators or fail parsing.
fn fts_query(query: &str) -> String {
    query.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| format!("\"{word}\""))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Pinned Notes come first; creation order is stable inside each group, and the
/// id breaks ties so every adapter and every restart agrees.
fn sort_notes(notes: &mut [Note]) {
    notes.sort_by(|left, right| {
        (!left.pinned, &left.created_at, &left.id).cmp(&(
            !right.pinned,
            &right.created_at,
            &right.id,
        ))
    });
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
        (3_i64, include_str!("../migrations/0003_note_controls.sql")),
        (4_i64, include_str!("../migrations/0004_labels_and_search.sql")),
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

fn labels_for_note(connection: &Connection, note_id: &str) -> Result<Vec<Label>, WorkspaceError> {
    connection.prepare("SELECT labels.id, labels.workspace_id, labels.name FROM labels JOIN note_labels ON note_labels.label_id = labels.id WHERE note_labels.note_id = ?1 ORDER BY labels.name, labels.id")
        .map_err(WorkspaceError::Storage)?.query_map([note_id], |row| Ok(Label { id: row.get(0)?, workspace_id: row.get(1)?, name: row.get(2)? }))
        .map_err(WorkspaceError::Storage)?.collect::<Result<Vec<_>, _>>().map_err(WorkspaceError::Storage)
}

fn refresh_workspace_search(connection: &Connection, workspace_id: &str) -> Result<(), WorkspaceError> {
    connection.execute("DELETE FROM note_search WHERE workspace_id = ?1", [workspace_id]).map_err(WorkspaceError::Storage)?;
    connection.execute("INSERT INTO note_search(note_id, workspace_id, content) SELECT notes.id, notes.workspace_id, notes.markdown || ' ' || COALESCE(notes.annotation, '') || ' ' || COALESCE((SELECT group_concat(labels.name, ' ') FROM note_labels JOIN labels ON labels.id = note_labels.label_id WHERE note_labels.note_id = notes.id), '') FROM notes WHERE notes.workspace_id = ?1", [workspace_id]).map_err(WorkspaceError::Storage)?;
    Ok(())
}

fn read_snapshot(connection: &Connection) -> Result<WorkspaceSnapshot, WorkspaceError> {
    let workspaces = read_workspaces(connection)?;
    let mut notes = connection.prepare("SELECT id, workspace_id, markdown, note_type, note_type_provenance, annotation, annotation_provenance, created_at, updated_at, pinned FROM notes")
        .map_err(WorkspaceError::Storage)?
        .query_map([], |row| Ok(Note {
            id: row.get(0)?,
            workspace_id: row.get(1)?,
            markdown: row.get(2)?,
            note_type: row.get(3)?,
            note_type_provenance: Provenance::from_str(&row.get::<_, String>(4)?),
            annotation: row.get(5)?,
            annotation_provenance: Provenance::from_str(&row.get::<_, String>(6)?),
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
            pinned: row.get::<_, i64>(9)? != 0,
            labels: vec![],
        }))
        .map_err(WorkspaceError::Storage)?.collect::<Result<Vec<_>, _>>().map_err(WorkspaceError::Storage)?;
    for note in &mut notes { note.labels = labels_for_note(connection, &note.id)?; }
    sort_notes(&mut notes);
    let active_workspace_id = read_active_workspace_id(connection)?
        .filter(|id| workspaces.iter().any(|workspace| &workspace.id == id))
        .or_else(|| workspaces.first().map(|workspace| workspace.id.clone()))
        .unwrap_or_default();
    Ok(WorkspaceSnapshot {
        workspaces,
        notes,
        active_workspace_id,
        undoable_commands: 0,
    })
}

/// Removes a database and its sidecars. Only ever called for a path that held
/// no database before the failed open that created the stub.
fn discard_database_files(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

fn id() -> String {
    Uuid::new_v4().to_string()
}

/// Fixed-width UTC, so lexicographic order is chronological order everywhere
/// timestamps are compared: SQL `ORDER BY`, survivor selection, and tests.
fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true)
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
        history: UndoHistory,
        labels: Vec<Label>,
    }

    impl MemoryStore {
        fn new() -> Self {
            let mut store = Self {
                workspaces: vec![],
                notes: vec![],
                active_workspace_id: String::new(),
                history: UndoHistory::default(),
                labels: vec![],
            };
            store.create_workspace(DEFAULT_WORKSPACE_NAME).unwrap();
            store
        }
    }

    impl ThinkingWorkspaceInterface for MemoryStore {
        fn snapshot(&self) -> Result<WorkspaceSnapshot, WorkspaceError> {
            let mut notes = self.notes.clone();
            sort_notes(&mut notes);
            Ok(WorkspaceSnapshot {
                workspaces: self.workspaces.clone(),
                notes,
                active_workspace_id: self.active_workspace_id.clone(),
                undoable_commands: self.history.depth(&self.active_workspace_id),
            })
        }

        fn history(&mut self) -> &mut UndoHistory {
            &mut self.history
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

        fn remove_workspace(
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

        fn apply_note_mutation(&mut self, mutation: &NoteMutation) -> Result<(), WorkspaceError> {
            match mutation {
                NoteMutation::Insert(note) => {
                    require_workspace(&self.workspaces, &note.workspace_id)?;
                    self.notes.push(note.clone());
                }
                NoteMutation::Replace(note) => {
                    let existing = self
                        .notes
                        .iter_mut()
                        .find(|candidate| candidate.id == note.id)
                        .ok_or(WorkspaceError::NoteNotFound)?;
                    *existing = note.clone();
                }
                NoteMutation::Delete { note_id } => {
                    let before = self.notes.len();
                    self.notes.retain(|note| &note.id != note_id);
                    if self.notes.len() == before {
                        return Err(WorkspaceError::NoteNotFound);
                    }
                }
            }
            Ok(())
        }

        fn attach_label(&mut self, note_id: &str, name: &str) -> Result<WorkspaceSnapshot, WorkspaceError> {
            let workspace_id = self.note(note_id)?.workspace_id;
            let (name, canonical) = validated_label_name(name)?;
            let label = self.labels.iter().find(|label| label.workspace_id == workspace_id && label.name.to_lowercase() == canonical).cloned().unwrap_or_else(|| {
                let label = Label { id: id(), workspace_id: workspace_id.clone(), name };
                self.labels.push(label.clone()); label
            });
            let note = self.notes.iter_mut().find(|note| note.id == note_id).ok_or(WorkspaceError::NoteNotFound)?;
            if !note.labels.iter().any(|candidate| candidate.id == label.id) { note.labels.push(label); }
            self.snapshot()
        }

        fn detach_label(&mut self, note_id: &str, label_id: &str) -> Result<WorkspaceSnapshot, WorkspaceError> {
            let note = self.notes.iter_mut().find(|note| note.id == note_id).ok_or(WorkspaceError::NoteNotFound)?;
            note.labels.retain(|label| label.id != label_id);
            self.labels.retain(|label| label.id != label_id || self.notes.iter().any(|note| note.labels.iter().any(|candidate| candidate.id == label_id)));
            self.snapshot()
        }

        fn rename_label(&mut self, label_id: &str, name: &str) -> Result<WorkspaceSnapshot, WorkspaceError> {
            let (name, canonical) = validated_label_name(name)?;
            let original = self.labels.iter().find(|label| label.id == label_id).cloned().ok_or(WorkspaceError::LabelNotFound)?;
            let collision = self.labels.iter().find(|label| label.id != label_id && label.workspace_id == original.workspace_id && label.name.to_lowercase() == canonical).cloned();
            if let Some(survivor) = collision {
                for note in &mut self.notes { if note.labels.iter().any(|label| label.id == label_id) && !note.labels.iter().any(|label| label.id == survivor.id) { note.labels.push(survivor.clone()); } note.labels.retain(|label| label.id != label_id); }
                self.labels.retain(|label| label.id != label_id);
            } else {
                for label in &mut self.labels { if label.id == label_id { label.name = name.clone(); } }
                for note in &mut self.notes { for label in &mut note.labels { if label.id == label_id { label.name = name.clone(); } } }
            }
            self.snapshot()
        }

        fn remove_label(&mut self, label_id: &str) -> Result<WorkspaceSnapshot, WorkspaceError> {
            if !self.labels.iter().any(|label| label.id == label_id) { return Err(WorkspaceError::LabelNotFound); }
            self.labels.retain(|label| label.id != label_id);
            for note in &mut self.notes { note.labels.retain(|label| label.id != label_id); }
            self.snapshot()
        }

        fn search_notes(&self, workspace_id: &str, query: &str) -> Result<Vec<SearchResult>, WorkspaceError> {
            require_workspace(&self.workspaces, workspace_id)?;
            let terms: Vec<_> = query.split(|character: char| !character.is_alphanumeric()).filter(|word| !word.is_empty()).map(str::to_lowercase).collect();
            if terms.is_empty() { return Ok(vec![]); }
            let mut results: Vec<_> = self.notes.iter().filter(|note| note.workspace_id == workspace_id).filter_map(|note| {
                let content = format!("{} {} {}", note.markdown, note.annotation.as_deref().unwrap_or(""), note.labels.iter().map(|label| label.name.as_str()).collect::<Vec<_>>().join(" "));
                let lower = content.to_lowercase();
                terms.iter().all(|term| lower.contains(term)).then(|| SearchResult { note_id: note.id.clone(), snippet: content.chars().take(160).collect(), note_type: note.note_type.clone(), labels: note.labels.clone(), rank: 0.0 })
            }).collect();
            results.sort_by(|left, right| left.note_id.cmp(&right.note_id));
            Ok(results)
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

    fn note_in(snapshot: &WorkspaceSnapshot, note_id: &str) -> Note {
        snapshot
            .notes
            .iter()
            .find(|note| note.id == note_id)
            .unwrap_or_else(|| panic!("no Note {note_id}"))
            .clone()
    }

    fn is_nothing_to_undo(result: &WorkspaceCommandResult) -> bool {
        matches!(
            result,
            WorkspaceCommandResult::Failed {
                failure: WorkspaceFailure {
                    code: WorkspaceFailureCode::NothingToUndo,
                    ..
                }
            }
        )
    }

    /// Every manual Note control and its compensating undo, at the interface.
    /// `workspace_id` must be the active Workspace and `note_id` a Note in it.
    fn note_control_conformance(
        workspace: &mut impl ThinkingWorkspaceInterface,
        workspace_id: &str,
        note_id: &str,
    ) {
        let original = note_in(&committed(workspace.snapshot_outcome()), note_id);

        // Text edits keep identity and creation order and are reversible.
        let edited = committed(workspace.edit_note_text_outcome(note_id, "  # Revised thought  "));
        assert_eq!(note_in(&edited, note_id).markdown, "  # Revised thought  ");
        assert_eq!(note_in(&edited, note_id).created_at, original.created_at);
        assert!(is_validation_failure(
            &workspace.edit_note_text_outcome(note_id, " \n\t ")
        ));
        assert!(is_not_found_failure(
            &workspace.edit_note_text_outcome("missing", "Nope")
        ));
        let undone = committed(workspace.undo_outcome(workspace_id));
        assert_eq!(note_in(&undone, note_id), original);

        // Every fixed Note Type is accepted and records manual authorship.
        for note_type in NOTE_TYPES {
            let typed = committed(workspace.set_note_type_outcome(note_id, note_type));
            let note = note_in(&typed, note_id);
            assert_eq!(note.note_type, note_type);
            assert_eq!(note.note_type_provenance, Provenance::Manual);
        }
        assert!(is_validation_failure(
            &workspace.set_note_type_outcome(note_id, "todo")
        ));
        assert!(is_validation_failure(
            &workspace.set_note_type_outcome(note_id, "CLAIM")
        ));
        let untyped = committed(workspace.undo_outcome(workspace_id));
        assert_eq!(
            note_in(&untyped, note_id).note_type,
            NOTE_TYPES[NOTE_TYPES.len() - 2]
        );
        committed(workspace.set_note_type_outcome(note_id, "question"));

        // Annotation is written, edited, and cleared independently of the text.
        let annotated =
            committed(workspace.set_note_annotation_outcome(note_id, "  Where did this come from? 🧠  "));
        let note = note_in(&annotated, note_id);
        assert_eq!(note.annotation.as_deref(), Some("Where did this come from? 🧠"));
        assert_eq!(note.annotation_provenance, Provenance::Manual);
        assert_eq!(note.markdown, original.markdown);
        assert_eq!(note.note_type, "question");
        let bound = "🧠".repeat(MAX_ANNOTATION_SCALARS);
        assert_eq!(
            note_in(
                &committed(workspace.set_note_annotation_outcome(note_id, &bound)),
                note_id
            )
            .annotation
            .as_deref(),
            Some(bound.as_str())
        );
        assert!(is_validation_failure(&workspace.set_note_annotation_outcome(
            note_id,
            &"🧠".repeat(MAX_ANNOTATION_SCALARS + 1)
        )));
        let cleared = committed(workspace.set_note_annotation_outcome(note_id, "   "));
        assert_eq!(note_in(&cleared, note_id).annotation, None);
        // Undo restores the previous Annotation rather than the Note text.
        let restored = committed(workspace.undo_outcome(workspace_id));
        assert_eq!(note_in(&restored, note_id).annotation.as_deref(), Some(bound.as_str()));

        // Pinned Notes sort first while creation order holds inside each group.
        let second = committed(workspace.create_note_outcome(workspace_id, "Second thought"));
        let second_id = second
            .notes
            .iter()
            .find(|note| note.markdown == "Second thought")
            .expect("the second Note is committed")
            .id
            .clone();
        let ordered = |snapshot: &WorkspaceSnapshot| -> Vec<String> {
            snapshot
                .notes
                .iter()
                .filter(|note| note.workspace_id == workspace_id)
                .map(|note| note.id.clone())
                .collect()
        };
        assert_eq!(ordered(&second), vec![note_id.to_owned(), second_id.clone()]);
        let pinned = committed(workspace.set_note_pinned_outcome(&second_id, true));
        assert!(note_in(&pinned, &second_id).pinned);
        assert_eq!(ordered(&pinned), vec![second_id.clone(), note_id.to_owned()]);
        let unpinned = committed(workspace.undo_outcome(workspace_id));
        assert!(!note_in(&unpinned, &second_id).pinned);
        assert_eq!(ordered(&unpinned), vec![note_id.to_owned(), second_id.clone()]);

        // Deleting a Note is reversible with the same identity and fields.
        let before_delete = note_in(&committed(workspace.snapshot_outcome()), &second_id);
        let after_delete = committed(workspace.delete_note_outcome(&second_id));
        assert!(!after_delete.notes.iter().any(|note| note.id == second_id));
        assert!(is_not_found_failure(&workspace.delete_note_outcome(&second_id)));
        let after_undo = committed(workspace.undo_outcome(workspace_id));
        assert_eq!(note_in(&after_undo, &second_id), before_delete);

        // An undone create removes the Note again, and history is per Workspace.
        let elsewhere = workspace_id_named(
            &committed(workspace.create_workspace_outcome("Undo isolation")),
            "Undo isolation",
        );
        assert!(is_nothing_to_undo(&workspace.undo_outcome(&elsewhere)));
        committed(workspace.create_note_outcome(&elsewhere, "Only reversible here"));
        let isolated = committed(workspace.undo_outcome(&elsewhere));
        assert!(!isolated
            .notes
            .iter()
            .any(|note| note.workspace_id == elsewhere));
        assert!(is_nothing_to_undo(&workspace.undo_outcome(&elsewhere)));
        // The other Workspace's history is untouched by all of that: its next
        // undo is still the create of the second Note.
        let after_isolated_undo = committed(workspace.undo_outcome(workspace_id));
        assert!(!after_isolated_undo
            .notes
            .iter()
            .any(|note| note.id == second_id));

        // History is bounded, so only the newest 20 commands stay reversible.
        committed(workspace.select_workspace_outcome(&elsewhere));
        let mut deep = committed(workspace.create_note_outcome(&elsewhere, "Overflow"));
        let overflow_id = deep
            .notes
            .iter()
            .find(|note| note.markdown == "Overflow")
            .expect("the Note is committed")
            .id
            .clone();
        for step in 0..MAX_REVERSIBLE_COMMANDS + 5 {
            deep = committed(workspace.edit_note_text_outcome(&overflow_id, &format!("Edit {step}")));
        }
        assert_eq!(deep.undoable_commands, MAX_REVERSIBLE_COMMANDS);
        for _ in 0..MAX_REVERSIBLE_COMMANDS {
            committed(workspace.undo_outcome(&elsewhere));
        }
        let exhausted = committed(workspace.snapshot_outcome());
        assert_eq!(exhausted.undoable_commands, 0);
        // The create and the oldest edits fell out of the bound, so the Note is
        // still here with the oldest text the bound could still reach.
        assert_eq!(note_in(&exhausted, &overflow_id).markdown, "Edit 4");
        assert!(is_nothing_to_undo(&workspace.undo_outcome(&elsewhere)));
        assert_eq!(committed(workspace.snapshot_outcome()), exhausted);

        // Deleting a Workspace drops its history, because its Notes are gone.
        committed(workspace.create_note_outcome(&elsewhere, "Goes with its Workspace"));
        committed(workspace.delete_workspace_outcome(&elsewhere));
        assert!(is_nothing_to_undo(&workspace.undo_outcome(&elsewhere)));

        // The surviving Workspace still undoes its own most recent change.
        committed(workspace.select_workspace_outcome(workspace_id));
        let temporary = committed(workspace.create_note_outcome(workspace_id, "Temporary thought"));
        let temporary_id = temporary
            .notes
            .iter()
            .find(|note| note.markdown == "Temporary thought")
            .expect("the Note is committed")
            .id
            .clone();
        committed(workspace.delete_note_outcome(&temporary_id));
        assert!(committed(workspace.undo_outcome(workspace_id))
            .notes
            .iter()
            .any(|note| note.id == temporary_id));
        committed(workspace.delete_note_outcome(&temporary_id));
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
        let second_research = duplicates[1].id.clone();

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
        let note_id = with_note.notes[0].id.clone();
        assert_eq!(with_note.notes[0].note_type_provenance, Provenance::Default);
        assert_eq!(with_note.notes[0].annotation, None);
        // Creating a Note is itself reversible in the Workspace that received it.
        assert_eq!(with_note.undoable_commands, 1);

        note_control_conformance(&mut workspace, &research, &note_id);

        let labeled = committed(workspace.attach_label_outcome(&note_id, "  Rêverie  "));
        let label = note_in(&labeled, &note_id).labels[0].clone();
        assert_eq!(label.name, "Rêverie");
        // Case and surrounding whitespace reuse the same Workspace-local Label.
        let same = committed(workspace.attach_label_outcome(&note_id, "rÊVERIE"));
        assert_eq!(note_in(&same, &note_id).labels.len(), 1);
        let second_note = committed(workspace.create_note_outcome(&research, "Searchable punctuation: C++ & Rust"));
        let second_note_id = second_note.notes.iter().find(|note| note.markdown.starts_with("Searchable")).unwrap().id.clone();
        committed(workspace.attach_label_outcome(&second_note_id, "Reading"));
        let merged = committed(workspace.rename_label_outcome(&label.id, "reading"));
        assert_eq!(note_in(&merged, &note_id).labels[0].name, "Reading");
        assert_eq!(note_in(&merged, &second_note_id).labels[0].name, "Reading");
        assert_eq!(workspace.search_notes(&research, "reading").unwrap().len(), 2);
        assert_eq!(workspace.search_notes(&research, "C++ &").unwrap().len(), 1);
        assert!(workspace.search_notes(&research, "   ").unwrap().is_empty());
        let isolated = committed(workspace.create_workspace_outcome("Elsewhere"));
        let elsewhere = isolated.active_workspace_id;
        let outside = committed(workspace.create_note_outcome(&elsewhere, "Reading must stay private"));
        let outside_id = outside.notes.iter().find(|note| note.workspace_id == elsewhere).unwrap().id.clone();
        committed(workspace.attach_label_outcome(&outside_id, "Reading"));
        assert_eq!(workspace.search_notes(&research, "reading").unwrap().len(), 2);
        let detached = committed(workspace.detach_label_outcome(&note_id, &note_in(&merged, &note_id).labels[0].id));
        assert!(note_in(&detached, &note_id).labels.is_empty());

        // Deleting the active Workspace selects the most recently updated
        // survivor. The survivor here is neither the newest, the oldest, the
        // first, nor the last Workspace, so no weaker rule can pass this.
        distinct_moment();
        let newest = workspace_id_named(
            &committed(workspace.create_workspace_outcome("Newest Workspace")),
            "Newest Workspace",
        );
        distinct_moment();
        committed(workspace.rename_workspace_outcome(&second_research, "Touched most recently"));
        committed(workspace.select_workspace_outcome(&research));
        let after_delete = committed(workspace.delete_workspace_outcome(&research));
        assert_eq!(after_delete.active_workspace_id, second_research);
        assert_eq!(after_delete.workspaces.len(), 4);
        assert_eq!(after_delete.workspaces[0].name, DEFAULT_WORKSPACE_NAME);
        assert_eq!(after_delete.workspaces[3].id, newest);
        assert!(after_delete.notes.iter().all(|note| note.workspace_id != research));

        // Deleting an inactive Workspace leaves the selection alone.
        let after_inactive_delete = committed(workspace.delete_workspace_outcome(&newest));
        assert_eq!(after_inactive_delete.active_workspace_id, second_research);

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
        discard_database_files(path);
    }

    fn database_files_present(path: &std::path::Path) -> bool {
        ["", "-wal", "-shm"]
            .iter()
            .any(|suffix| std::path::Path::new(&format!("{}{suffix}", path.display())).exists())
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
    fn sqlite_recovers_labels_and_fts_search_after_reopen() {
        let path = temporary_path();
        let workspace_id = {
            let mut store = WorkspaceStore::open(&path).unwrap();
            let workspace_id = store.snapshot().unwrap().active_workspace_id;
            let note = store.create_note(&workspace_id, "A Unicode 🧠 search target").unwrap().notes[0].id.clone();
            store.set_note_annotation(&note, "Contextual commentary").unwrap();
            store.attach_label(&note, "Rêverie").unwrap();
            workspace_id
        };
        let reopened = WorkspaceStore::open(&path).unwrap();
        let labels = reopened.search_notes(&workspace_id, "rêverie").unwrap();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].labels[0].name, "Rêverie");
        assert_eq!(reopened.search_notes(&workspace_id, "commentary").unwrap().len(), 1);
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

        // A selection that no longer exists — only reachable by tampering from
        // outside the interface — falls back to a valid Workspace.
        {
            let connection = Connection::open(&path).unwrap();
            write_active_workspace_id(&connection, "vanished").unwrap();
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

        // A rejected rename leaves the name and the selection intact.
        let before_rename = store.snapshot().unwrap();
        store.connection.execute_batch("CREATE TRIGGER reject_renames BEFORE UPDATE ON thinking_workspaces BEGIN SELECT RAISE(FAIL, 'injected'); END;").unwrap();
        assert!(matches!(
            store.rename_workspace_outcome(&workspace, "Must not persist"),
            WorkspaceCommandResult::Failed { .. }
        ));
        assert_eq!(store.snapshot().unwrap(), before_rename);
        store
            .connection
            .execute_batch("DROP TRIGGER reject_renames;")
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

    /// An existing database predates Annotation and the wider Note Type set, so
    /// the migration must carry its Notes forward instead of dropping them.
    #[test]
    fn the_note_controls_migration_preserves_notes_written_by_an_earlier_version() {
        let path = temporary_path();
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(include_str!("../migrations/0001_initial.sql"))
                .unwrap();
            connection
                .execute_batch(include_str!("../migrations/0002_preferences.sql"))
                .unwrap();
            connection.execute_batch("CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY NOT NULL, applied_at TEXT NOT NULL); INSERT INTO schema_migrations VALUES (1, 'then'), (2, 'then');").unwrap();
            connection
                .execute(
                    "INSERT INTO thinking_workspaces (id, name, created_at, updated_at) VALUES ('w', 'Earlier', 'then', 'then')",
                    [],
                )
                .unwrap();
            connection.execute("INSERT INTO notes (id, workspace_id, markdown, note_type, created_at, updated_at, pinned) VALUES ('n', 'w', 'Written before Annotation existed', 'general', 'then', 'then', 1)", []).unwrap();
        }

        let snapshot = WorkspaceStore::open(&path).unwrap().snapshot().unwrap();
        let note = snapshot.notes.iter().find(|note| note.id == "n").unwrap();
        assert_eq!(note.markdown, "Written before Annotation existed");
        assert_eq!(note.note_type, DEFAULT_NOTE_TYPE);
        assert_eq!(note.note_type_provenance, Provenance::Default);
        assert_eq!(note.annotation, None);
        assert_eq!(note.annotation_provenance, Provenance::Default);
        assert!(note.pinned);
        remove_database(&path);
    }

    #[test]
    fn manual_note_control_survives_reopen_while_undo_history_does_not() {
        let path = temporary_path();
        let (workspace_id, note_id, other_id) = {
            let mut store = WorkspaceStore::open(&path).unwrap();
            let workspace_id = store.snapshot().unwrap().active_workspace_id;
            let note_id = store
                .create_note(&workspace_id, "# Original")
                .unwrap()
                .notes[0]
                .id
                .clone();
            store.edit_note_text(&note_id, "# Edited *in place*").unwrap();
            store.set_note_type(&note_id, "thesis").unwrap();
            store
                .set_note_annotation(&note_id, "Source: a walk 🧠")
                .unwrap();
            store.set_note_pinned(&note_id, true).unwrap();
            let other_id = store
                .create_note(&workspace_id, "Unpinned and later")
                .unwrap()
                .notes
                .iter()
                .find(|note| note.markdown == "Unpinned and later")
                .unwrap()
                .id
                .clone();
            assert!(store.snapshot().unwrap().undoable_commands > 0);
            (workspace_id, note_id, other_id)
        };

        let reopened = WorkspaceStore::open(&path).unwrap();
        let snapshot = reopened.snapshot().unwrap();
        let note = snapshot
            .notes
            .iter()
            .find(|note| note.id == note_id)
            .unwrap();
        assert_eq!(note.markdown, "# Edited *in place*");
        assert_eq!(note.note_type, "thesis");
        assert_eq!(note.note_type_provenance, Provenance::Manual);
        assert_eq!(note.annotation.as_deref(), Some("Source: a walk 🧠"));
        assert_eq!(note.annotation_provenance, Provenance::Manual);
        assert!(note.pinned);
        // Pinned first, then creation order, identically after a restart.
        assert_eq!(
            snapshot
                .notes
                .iter()
                .map(|note| note.id.clone())
                .collect::<Vec<_>>(),
            vec![note_id, other_id]
        );

        // Restart cleared undo without changing anything durable.
        assert_eq!(snapshot.undoable_commands, 0);
        let mut reopened = reopened;
        assert!(matches!(
            reopened.undo_outcome(&workspace_id),
            WorkspaceCommandResult::Failed {
                failure: WorkspaceFailure {
                    code: WorkspaceFailureCode::NothingToUndo,
                    ..
                }
            }
        ));
        assert_eq!(reopened.snapshot().unwrap(), snapshot);
        drop(reopened);
        remove_database(&path);
    }

    #[test]
    fn a_failed_note_edit_or_undo_changes_neither_storage_nor_undo_availability() {
        let path = temporary_path();
        let mut store = WorkspaceStore::open(&path).unwrap();
        let workspace_id = store.snapshot().unwrap().active_workspace_id;
        let note_id = store.create_note(&workspace_id, "Committed").unwrap().notes[0]
            .id
            .clone();
        store.set_note_type(&note_id, "claim").unwrap();
        let before = store.snapshot().unwrap();

        store.connection.execute_batch("CREATE TRIGGER reject_note_updates BEFORE UPDATE ON notes BEGIN SELECT RAISE(FAIL, 'injected'); END;").unwrap();
        assert!(matches!(
            store.edit_note_text_outcome(&note_id, "Must not persist"),
            WorkspaceCommandResult::Failed {
                failure: WorkspaceFailure {
                    code: WorkspaceFailureCode::Storage,
                    ..
                }
            }
        ));
        // A rejected edit leaves the Note and the reversible history alone.
        assert_eq!(store.snapshot().unwrap(), before);

        // The pending undo is the Note Type change, which the same trigger
        // rejects. A failed undo must keep that step reversible.
        assert!(matches!(
            store.undo_outcome(&workspace_id),
            WorkspaceCommandResult::Failed {
                failure: WorkspaceFailure {
                    code: WorkspaceFailureCode::Storage,
                    ..
                }
            }
        ));
        assert_eq!(store.snapshot().unwrap(), before);
        store
            .connection
            .execute_batch("DROP TRIGGER reject_note_updates;")
            .unwrap();
        let undone = committed(store.undo_outcome(&workspace_id));
        assert_eq!(undone.notes[0].note_type, DEFAULT_NOTE_TYPE);
        assert_eq!(undone.notes[0].note_type_provenance, Provenance::Default);

        // A rejected delete leaves the Note and its history intact too.
        let before_delete = store.snapshot().unwrap();
        store.connection.execute_batch("CREATE TRIGGER reject_note_deletes BEFORE DELETE ON notes BEGIN SELECT RAISE(FAIL, 'injected'); END;").unwrap();
        assert!(matches!(
            store.delete_note_outcome(&note_id),
            WorkspaceCommandResult::Failed { .. }
        ));
        assert_eq!(store.snapshot().unwrap(), before_delete);
        store
            .connection
            .execute_batch("DROP TRIGGER reject_note_deletes;")
            .unwrap();
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
    fn a_failed_open_on_a_fresh_path_leaves_no_database_behind() {
        let path = temporary_path();
        assert!(!database_files_present(&path));
        let failure = WorkspaceStore::open_prepared(&path, |_| {
            Err(WorkspaceError::Storage(rusqlite::Error::InvalidQuery))
        })
        .unwrap_err();
        assert_eq!(failure.category, StorageOpenFailureCategory::Migration);
        // The stub SQLite created before preparation failed is gone, so the next
        // launch cannot mistake an empty stub for a legitimate fresh database.
        assert!(!database_files_present(&path));
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
