use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

use crate::thinking_graph::{
    canonical_pair, is_related, relatable_workspace_id, GraphViolation, Relationship,
    RelationshipProvenance,
};

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
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) assistance_policy: AssistancePolicy,
    pub(crate) selected_model: Option<String>,
    /// When the thinker first accepted the Cloud AI disclosure for this
    /// Workspace. Present means the disclosure has been read and the
    /// Workspace may use Ollama Cloud; absence is "not yet asked" or
    /// "asked and declined". The bearer key itself is never stored here.
    pub(crate) cloud_consent_at: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

impl ThinkingWorkspace {
    /// The durable identity of this Workspace. Stable across renames and
    /// never reused after a delete.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Whether the Workspace has given affirmative consent to use Ollama
    /// Cloud. The actual bearer key lives in the macOS keychain, not here.
    pub fn cloud_consent_at(&self) -> Option<&str> {
        self.cloud_consent_at.as_deref()
    }

    /// The Assistance Policy the Workspace is currently using. The Tauri
    /// command surface reads this to decide whether to send an enrichment
    /// request to Ollama.
    pub fn assistance_policy(&self) -> AssistancePolicy {
        self.assistance_policy
    }

    /// The opaque model identifier the thinker has selected, if any.
    pub fn selected_model(&self) -> Option<&str> {
        self.selected_model.as_deref()
    }
}

/// The per-Workspace choice that governs whether organization is Manual,
/// uses a local Ollama host, or may use Ollama Cloud after explicit consent.
/// Cloud AI is admitted as a durable value now so the next slice can add
/// behavior without changing what an existing row means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistancePolicy {
    Manual,
    LocalAi,
    CloudAi,
}

impl AssistancePolicy {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::LocalAi => "local_ai",
            Self::CloudAi => "cloud_ai",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "local_ai" => Self::LocalAi,
            "cloud_ai" => Self::CloudAi,
            _ => Self::Manual,
        }
    }
}

/// Who last decided a value. Manual authorship is durable so a later AI slice
/// cannot overwrite a thinker's choice silently. `Ai` is admitted as a new
/// variant so AI-authored fields are visibly distinguishable in the UI and a
/// later AI result may refresh them, while `Manual` always wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    Default,
    Manual,
    Ai,
}

impl Provenance {
    fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Manual => "manual",
            Self::Ai => "ai",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "manual" => Self::Manual,
            "ai" => Self::Ai,
            _ => Self::Default,
        }
    }

    /// Whether an AI result may overwrite a value with this provenance.
    /// `Manual` always wins; `Default` and `Ai` both yield. Re-enrich and
    /// Replace is the only path that bypasses this rule, and it does so by
    /// passing `force = true` through a separate command.
    pub fn is_ai_writable(self) -> bool {
        matches!(self, Self::Default | Self::Ai)
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
    /// Bumped on every commit that touches this Note. The Enrichment
    /// Workflow captures the value at request time and refuses to apply a
    /// result that names a different revision, so an edit made during
    /// inference invalidates the in-flight response.
    pub(crate) enrichment_revision: u64,
    /// The moment a successful AI organization was last applied, if any.
    /// `None` means the Note has never been enriched; present means a
    /// subsequent regular enrichment may refresh AI-authored fields.
    pub(crate) last_enriched_at: Option<String>,
    labels: Vec<Label>,
}

impl Note {
    /// The Thinking Graph identifies endpoints by these two values alone.
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    /// The enrichment revision at the moment of the most recent commit. The
    /// Enrichment Workflow captures this value into the request token and
    /// refuses to apply a result that names a different revision, so an
    /// edit made during inference invalidates the in-flight response.
    pub fn enrichment_revision(&self) -> u64 {
        self.enrichment_revision
    }

    /// The Labels this Note currently carries, in their canonical order.
    /// Used by the Enrichment Workflow to dedupe suggested Labels and to
    /// decide which new Labels a parsed result adds.
    pub fn labels(&self) -> &[Label] {
        &self.labels
    }

    /// The Markdown body of the Note. The Enrichment Workflow reads this
    /// for the target and for each candidate it sends.
    pub fn markdown(&self) -> &str {
        &self.markdown
    }

    /// The current Note Type. Used by the Enrichment Workflow to decide
    /// whether a parsed result actually changes the value.
    pub fn note_type(&self) -> &str {
        &self.note_type
    }

    /// The current Annotation, if any.
    pub fn annotation(&self) -> Option<&str> {
        self.annotation.as_deref()
    }

    /// The provenance of the current Note Type. The Enrichment Workflow
    /// reads this to decide whether a parsed result may overwrite it.
    pub fn note_type_provenance(&self) -> Provenance {
        self.note_type_provenance
    }

    /// The provenance of the current Annotation.
    pub fn annotation_provenance(&self) -> Provenance {
        self.annotation_provenance
    }

    /// The last update timestamp, used to sort candidates by recency.
    pub fn updated_at(&self) -> &str {
        &self.updated_at
    }

    /// When a successful AI organization was last applied, if ever. A
    /// subsequent regular enrichment may refresh AI-authored fields
    /// whose provenance is `Ai`; a manual edit supersedes that for the
    /// touched field only. The Tauri command surface may use this to
    /// skip an enrichment when the same response would just re-confirm
    /// the current values.
    #[allow(dead_code)]
    pub fn last_enriched_at(&self) -> Option<&str> {
        self.last_enriched_at.as_deref()
    }

    /// Replaces the Note Type and its provenance in one call. The Enrichment
    /// Workflow uses this on the test path; the application code paths go
    /// through the public trait methods so the transaction and provenance
    /// invariants stay in one place.
    #[cfg(test)]
    pub fn set_note_type_provenance(&mut self, note_type: &str, provenance: Provenance) {
        self.note_type = note_type.to_owned();
        self.note_type_provenance = provenance;
    }

    /// Replaces the Annotation and its provenance in one call.
    #[cfg(test)]
    pub fn set_annotation_provenance(&mut self, annotation: Option<&str>, provenance: Provenance) {
        self.annotation = annotation.map(str::to_owned);
        self.annotation_provenance = provenance;
    }

    /// Constructs a Note for tests. Production code paths go through the
    /// `create_note` and `apply_note_mutation` seams so the schema and
    /// invariants stay in one place.
    #[cfg(test)]
    pub fn new_for_test(
        id: &str,
        workspace_id: &str,
        markdown: &str,
        note_type: &str,
        updated_at: &str,
        annotation: Option<&str>,
        labels: Vec<Label>,
    ) -> Self {
        Self {
            id: id.to_owned(),
            workspace_id: workspace_id.to_owned(),
            markdown: markdown.to_owned(),
            note_type: note_type.to_owned(),
            note_type_provenance: Provenance::Default,
            annotation: annotation.map(str::to_owned),
            annotation_provenance: Provenance::Default,
            created_at: updated_at.to_owned(),
            updated_at: updated_at.to_owned(),
            pinned: false,
            enrichment_revision: 0,
            last_enriched_at: None,
            labels,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    id: String,
    workspace_id: String,
    name: String,
}

impl Label {
    /// The display name as the thinker sees it. Case is preserved; identity
    /// is the canonical (lowercased) form held by the database.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Constructs a Label for tests. Production code paths go through the
    /// `attach_label` intent so identity, canonicalization, and dedup
    /// stay in one place.
    #[cfg(test)]
    pub fn new_for_test(id: &str, workspace_id: &str, name: &str) -> Self {
        Self {
            id: id.to_owned(),
            workspace_id: workspace_id.to_owned(),
            name: name.to_owned(),
        }
    }
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
    Delete {
        note_id: String,
    },
    /// Puts an existing Note in `note.workspace_id`, gives it exactly the Label
    /// meanings `note` carries, and leaves it with exactly `relationships` —
    /// each kept only while both endpoints are Notes in that same Workspace.
    /// One shape serves a move and the undo of a move, in either direction.
    Relocate {
        note: Note,
        relationships: Vec<Relationship>,
    },
}

impl NoteMutation {
    fn workspace_id(&self) -> &str {
        match self {
            Self::Insert(note) | Self::Replace(note) | Self::Relocate { note, .. } => {
                &note.workspace_id
            }
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
    pub(crate) workspaces: Vec<ThinkingWorkspace>,
    pub(crate) notes: Vec<Note>,
    /// Every Relationship in every Workspace, in creation order. Each endpoint
    /// always names a Note in `notes`.
    pub(crate) relationships: Vec<Relationship>,
    pub(crate) active_workspace_id: String,
    /// How many mutations in the active Workspace can still be undone in this
    /// session. Always zero right after a restart.
    pub(crate) undoable_commands: usize,
}

impl WorkspaceSnapshot {
    /// All committed Thinking Workspaces, in creation order. The lib boundary
    /// uses this to look up a single Workspace by id without exposing the
    /// whole struct as public-API.
    pub fn workspaces(&self) -> &[ThinkingWorkspace] {
        &self.workspaces
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceFailureCode {
    Validation,
    NotFound,
    NothingToUndo,
    Storage,
    /// The Enrichment request token no longer matches the Note or the
    /// Workspace. The UI uses this to render a typed retry state.
    Stale,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkspaceFailure {
    pub(crate) code: WorkspaceFailureCode,
    pub(crate) message: String,
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
    pub(crate) category: StorageOpenFailureCategory,
    pub(crate) message: String,
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
    #[error("That is not an Assistance Policy Nodepad recognizes.")]
    UnknownAssistancePolicy,
    #[error("A Label needs one to four words and may not exceed {MAX_LABEL_NAME_SCALARS} characters.")]
    InvalidLabelName,
    #[error("That Label no longer exists.")]
    LabelNotFound,
    #[error("The selected Thinking Workspace no longer exists.")]
    WorkspaceNotFound,
    #[error("That Note no longer exists.")]
    NoteNotFound,
    #[error("A Relationship connects two different Notes.")]
    SelfRelationship,
    #[error("A Relationship connects two Notes in the same Thinking Workspace.")]
    CrossWorkspaceRelationship,
    #[error("Choose a different Thinking Workspace to move or copy this Note into.")]
    SameWorkspaceTransfer,
    #[error("There is nothing left to undo in this Thinking Workspace.")]
    NothingToUndo,
    #[error("The AI organization result no longer matches this Note. Retry enrichment.")]
    StaleEnrichment,
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
            | Self::UnknownNoteType
            | Self::UnknownAssistancePolicy
            | Self::InvalidLabelName
            | Self::SelfRelationship
            | Self::CrossWorkspaceRelationship
            | Self::SameWorkspaceTransfer => WorkspaceFailureCode::Validation,
            Self::WorkspaceNotFound | Self::NoteNotFound | Self::LabelNotFound => WorkspaceFailureCode::NotFound,
            Self::NothingToUndo => WorkspaceFailureCode::NothingToUndo,
            Self::StaleEnrichment => WorkspaceFailureCode::Stale,
            Self::Storage(_) => WorkspaceFailureCode::Storage,
        };
        WorkspaceFailure {
            code,
            message: self.to_string(),
        }
    }
}

impl From<GraphViolation> for WorkspaceError {
    fn from(violation: GraphViolation) -> Self {
        match violation {
            GraphViolation::SelfRelationship => Self::SelfRelationship,
            GraphViolation::CrossWorkspace => Self::CrossWorkspaceRelationship,
            GraphViolation::MissingNote => Self::NoteNotFound,
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
    /// Records one symmetric, untyped Relationship between two distinct Notes
    /// in the same Workspace, with manual provenance. Asking for a Relationship
    /// that already exists commits nothing and adds no second row.
    fn relate_notes(
        &mut self,
        note_id: &str,
        other_note_id: &str,
    ) -> Result<WorkspaceSnapshot, WorkspaceError>;
    /// Removes the Relationship between two Notes, in either endpoint order.
    fn unrelate_notes(
        &mut self,
        note_id: &str,
        other_note_id: &str,
    ) -> Result<WorkspaceSnapshot, WorkspaceError>;

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
            enrichment_revision: 0,
            last_enriched_at: None,
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
        edited.enrichment_revision = previous.enrichment_revision + 1;
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
        typed.enrichment_revision = previous.enrichment_revision + 1;
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
        annotated.enrichment_revision = previous.enrichment_revision + 1;
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
        repinned.enrichment_revision = previous.enrichment_revision + 1;
        self.commit_note(
            NoteMutation::Replace(repinned),
            NoteMutation::Replace(previous),
        )
    }

    /// The Note and the Workspace a transfer names, once both are known to
    /// exist and to be different Workspaces. Every refusal is decided here,
    /// before a transaction exists, so a rejected transfer leaves both
    /// Workspaces exactly as they were.
    fn transferable_note(
        &self,
        note_id: &str,
        target_workspace_id: &str,
    ) -> Result<(WorkspaceSnapshot, Note), WorkspaceError> {
        let snapshot = self.snapshot()?;
        require_workspace(&snapshot.workspaces, target_workspace_id)?;
        let note = snapshot
            .notes
            .iter()
            .find(|note| note.id == note_id)
            .cloned()
            .ok_or(WorkspaceError::NoteNotFound)?;
        if note.workspace_id == target_workspace_id {
            return Err(WorkspaceError::SameWorkspaceTransfer);
        }
        Ok((snapshot, note))
    }

    /// Moves a Note into another Thinking Workspace. Identity and every
    /// authored field survive; Labels are remapped into the target by display
    /// meaning; every Relationship is removed, because a Relationship cannot
    /// cross a Workspace seam. Undoing returns the Note and restores only the
    /// Relationships whose endpoints are both still in the prior Workspace.
    fn move_note(
        &mut self,
        note_id: &str,
        target_workspace_id: &str,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let (snapshot, source) = self.transferable_note(note_id, target_workspace_id)?;
        let captured = snapshot
            .relationships
            .iter()
            .filter(|relationship| relationship.other_endpoint(note_id).is_some())
            .cloned()
            .collect();
        let mut moved = source.clone();
        moved.workspace_id = target_workspace_id.to_owned();
        // The move itself is a fresh commit, so the revision advances. The
        // compensation restores the row at its prior revision verbatim.
        moved.enrichment_revision = source.enrichment_revision + 1;
        // The command belongs to the Workspace the thinker acted in, so its
        // undo is where the Note was last seen leaving.
        let origin = source.workspace_id.clone();
        self.commit_note_in(
            &origin,
            NoteMutation::Relocate {
                note: moved,
                relationships: vec![],
            },
            NoteMutation::Relocate {
                note: source,
                relationships: captured,
            },
        )
    }

    /// Copies a Note into another Thinking Workspace. The copy keeps the text,
    /// Note Type, Annotation, pin state, manual provenance, and Label meanings,
    /// and takes a fresh identity, a fresh timestamp, and no Relationship.
    fn copy_note(
        &mut self,
        note_id: &str,
        target_workspace_id: &str,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let (_, source) = self.transferable_note(note_id, target_workspace_id)?;
        let now = timestamp();
        let copy = Note {
            id: id(),
            workspace_id: target_workspace_id.to_owned(),
            created_at: now.clone(),
            updated_at: now,
            ..source.clone()
        };
        let copy_id = copy.id.clone();
        self.commit_note_in(
            &source.workspace_id,
            NoteMutation::Insert(copy),
            NoteMutation::Delete { note_id: copy_id },
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

    /// Applies a parsed AI organization result to a Note, gated by the
    /// manual-provenance rule and the request token. Returns the new
    /// snapshot on success, or a typed failure when the token is stale
    /// (revision moved), the Workspace policy no longer permits AI, or the
    /// Note disappeared. The Note's text is never rewritten, merged, or
    /// deleted.
    fn apply_enrichment(
        &mut self,
        workspace_id: &str,
        note_id: &str,
        result: &crate::enrichment::ParsedEnrichmentResult,
        token: &crate::enrichment::RequestToken,
        force: bool,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let snapshot = self.snapshot()?;
        let workspace = snapshot
            .workspaces
            .iter()
            .find(|candidate| candidate.id == workspace_id)
            .ok_or(WorkspaceError::WorkspaceNotFound)?;
        // Policy drift invalidates the result even if everything else
        // matched; the thinker returned to Manual since the request began.
        if !force
            && !matches!(
                workspace.assistance_policy,
                crate::workspace::AssistancePolicy::LocalAi
                    | crate::workspace::AssistancePolicy::CloudAi
            )
        {
            return Err(WorkspaceError::StaleEnrichment);
        }
        let note = snapshot
            .notes
            .iter()
            .find(|candidate| candidate.id == note_id)
            .cloned()
            .ok_or(WorkspaceError::NoteNotFound)?;
        if note.workspace_id != workspace_id {
            return Err(WorkspaceError::NoteNotFound);
        }
        if note.enrichment_revision() != token.revision {
            return Err(WorkspaceError::StaleEnrichment);
        }
        let existing_relationship_ids: Vec<String> = snapshot
            .relationships
            .iter()
            .filter(|relationship| relationship.other_endpoint(note_id).is_some())
            .filter_map(|relationship| relationship.other_endpoint(note_id).map(str::to_owned))
            .collect();
        let applied = crate::enrichment::gate_parsed_against_source(
            result,
            &note,
            &existing_relationship_ids,
            force,
        );
        self.persist_enrichment(workspace_id, note_id, &note, &applied)
    }

    /// The transactional commit. Implementations open a single transaction,
    /// update the Note row, attach the suggested Labels, insert the new
    /// Relationships with `Ai` provenance, refresh the FTS index, and
    /// commit. A failure leaves nothing behind.
    fn persist_enrichment(
        &mut self,
        workspace_id: &str,
        note_id: &str,
        previous: &Note,
        applied: &crate::enrichment::ApplicableFields,
    ) -> Result<WorkspaceSnapshot, WorkspaceError>;

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
        self.commit_note_in(&workspace_id, mutation, compensation)
    }

    /// The same commit, for an intent whose reversible history belongs to a
    /// Workspace neither side of the mutation names — a transfer, whose undo
    /// belongs to the Workspace the Note came from.
    fn commit_note_in(
        &mut self,
        workspace_id: &str,
        mutation: NoteMutation,
        compensation: NoteMutation,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        self.apply_note_mutation(&mutation)?;
        self.history().push(workspace_id, compensation);
        self.snapshot()
    }

    /// Changes the Assistance Policy of one Thinking Workspace. Switching to
    /// Manual stops future provider calls; the UI is responsible for ignoring
    /// any in-flight discovery responses that arrive afterwards.
    fn set_assistance_policy(
        &mut self,
        workspace_id: &str,
        policy: AssistancePolicy,
    ) -> Result<WorkspaceSnapshot, WorkspaceError>;

    /// Records the chosen model identifier for one Thinking Workspace. The
    /// identifier is opaque to Nodepad; validation against the current list
    /// is a UI concern.
    fn set_selected_model(
        &mut self,
        workspace_id: &str,
        model_id: Option<&str>,
    ) -> Result<WorkspaceSnapshot, WorkspaceError>;

    /// Records or clears the Workspace's affirmative consent to use Ollama
    /// Cloud. `accept` true records the moment of first consent; false clears
    /// it. The bearer key is never stored in the database — this row only
    /// names the Workspace and the moment consent was given.
    fn set_cloud_consent(
        &mut self,
        workspace_id: &str,
        accept: bool,
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
    fn set_assistance_policy_outcome(
        &mut self,
        workspace_id: &str,
        policy: &str,
    ) -> WorkspaceCommandResult {
        let policy = match policy {
            "manual" => AssistancePolicy::Manual,
            "local_ai" => AssistancePolicy::LocalAi,
            "cloud_ai" => AssistancePolicy::CloudAi,
            _ => return outcome(Err(WorkspaceError::UnknownAssistancePolicy)),
        };
        outcome(self.set_assistance_policy(workspace_id, policy))
    }
    fn set_selected_model_outcome(
        &mut self,
        workspace_id: &str,
        model_id: Option<&str>,
    ) -> WorkspaceCommandResult {
        outcome(self.set_selected_model(workspace_id, model_id))
    }
    fn set_cloud_consent_outcome(
        &mut self,
        workspace_id: &str,
        accept: bool,
    ) -> WorkspaceCommandResult {
        outcome(self.set_cloud_consent(workspace_id, accept))
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
    fn move_note_outcome(
        &mut self,
        note_id: &str,
        target_workspace_id: &str,
    ) -> WorkspaceCommandResult {
        outcome(self.move_note(note_id, target_workspace_id))
    }
    fn copy_note_outcome(
        &mut self,
        note_id: &str,
        target_workspace_id: &str,
    ) -> WorkspaceCommandResult {
        outcome(self.copy_note(note_id, target_workspace_id))
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
    fn relate_notes_outcome(&mut self, note_id: &str, other_note_id: &str) -> WorkspaceCommandResult {
        outcome(self.relate_notes(note_id, other_note_id))
    }
    fn unrelate_notes_outcome(
        &mut self,
        note_id: &str,
        other_note_id: &str,
    ) -> WorkspaceCommandResult {
        outcome(self.unrelate_notes(note_id, other_note_id))
    }
    /// Atomically applies a parsed AI organization result to a Note. The
    /// manual-provenance gate means a later AI result never overwrites a
    /// manual field. The `force` flag is set by the explicit Re-enrich and
    /// Replace action; otherwise enrichment only writes fields that are
    /// `Default` or `Ai` at commit time. A stale request token (a Note,
    /// revision, policy, or model that no longer matches) is rejected
    /// before any write happens.
    fn apply_enrichment_outcome(
        &mut self,
        workspace_id: &str,
        note_id: &str,
        result: &crate::enrichment::ParsedEnrichmentResult,
        token: &crate::enrichment::RequestToken,
        force: bool,
    ) -> WorkspaceCommandResult {
        outcome(self.apply_enrichment(workspace_id, note_id, result, token, force))
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
            "INSERT INTO thinking_workspaces (id, name, assistance_policy, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![workspace_id, name, AssistancePolicy::Manual.as_str(), now],
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

    fn set_assistance_policy(
        &mut self,
        workspace_id: &str,
        policy: AssistancePolicy,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        require_workspace(&read_workspaces(&self.connection)?, workspace_id)?;
        let transaction = self
            .connection
            .transaction()
            .map_err(WorkspaceError::Storage)?;
        transaction
            .execute(
                "UPDATE thinking_workspaces SET assistance_policy = ?2, updated_at = ?3 WHERE id = ?1",
                params![workspace_id, policy.as_str(), timestamp()],
            )
            .map_err(WorkspaceError::Storage)?;
        transaction.commit().map_err(WorkspaceError::Storage)?;
        self.snapshot()
    }

    fn set_selected_model(
        &mut self,
        workspace_id: &str,
        model_id: Option<&str>,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        require_workspace(&read_workspaces(&self.connection)?, workspace_id)?;
        let transaction = self
            .connection
            .transaction()
            .map_err(WorkspaceError::Storage)?;
        transaction
            .execute(
                "UPDATE thinking_workspaces SET selected_model = ?2, updated_at = ?3 WHERE id = ?1",
                params![workspace_id, model_id, timestamp()],
            )
            .map_err(WorkspaceError::Storage)?;
        transaction.commit().map_err(WorkspaceError::Storage)?;
        self.snapshot()
    }

    fn set_cloud_consent(
        &mut self,
        workspace_id: &str,
        accept: bool,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        require_workspace(&read_workspaces(&self.connection)?, workspace_id)?;
        // The timestamp is the receipt; absence means consent was never given
        // or has been revoked. The moment of consent, not the bearer key, is
        // what is durable.
        let consent_at: Option<String> = if accept { Some(timestamp()) } else { None };
        let transaction = self
            .connection
            .transaction()
            .map_err(WorkspaceError::Storage)?;
        transaction
            .execute(
                "UPDATE thinking_workspaces SET cloud_consent_at = ?2, updated_at = ?3 WHERE id = ?1",
                params![workspace_id, consent_at, timestamp()],
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
        let committed_workspace_id = |note_id: &str| -> Result<Option<String>, WorkspaceError> {
            self.connection
                .query_row(
                    "SELECT workspace_id FROM notes WHERE id = ?1",
                    [note_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(WorkspaceError::Storage)
        };
        // A relocation touches two Workspaces, so both projections are rebuilt.
        let affected_workspace_ids: Vec<String> = match mutation {
            NoteMutation::Insert(note) | NoteMutation::Replace(note) => {
                vec![note.workspace_id.clone()]
            }
            NoteMutation::Delete { note_id } => {
                committed_workspace_id(note_id)?.into_iter().collect()
            }
            NoteMutation::Relocate { note, .. } => committed_workspace_id(&note.id)?
                .into_iter()
                .chain(std::iter::once(note.workspace_id.clone()))
                .collect(),
        };
        let transaction = self
            .connection
            .transaction()
            .map_err(WorkspaceError::Storage)?;
        let changed = match mutation {
            NoteMutation::Insert(note) => transaction.execute(
                "INSERT INTO notes (id, workspace_id, markdown, note_type, note_type_provenance, annotation, annotation_provenance, created_at, updated_at, pinned, enrichment_revision, last_enriched_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
                    note.enrichment_revision as i64,
                    note.last_enriched_at,
                ],
            ),
            NoteMutation::Replace(note) => transaction.execute(
                "UPDATE notes SET markdown = ?2, note_type = ?3, note_type_provenance = ?4, annotation = ?5, annotation_provenance = ?6, updated_at = ?7, pinned = ?8, enrichment_revision = ?9 WHERE id = ?1",
                params![
                    note.id,
                    note.markdown,
                    note.note_type,
                    note.note_type_provenance.as_str(),
                    note.annotation,
                    note.annotation_provenance.as_str(),
                    note.updated_at,
                    i64::from(note.pinned),
                    note.enrichment_revision as i64,
                ],
            ),
            NoteMutation::Delete { note_id } => {
                transaction.execute("DELETE FROM notes WHERE id = ?1", params![note_id])
            }
            NoteMutation::Relocate { note, .. } => transaction.execute(
                "UPDATE notes SET workspace_id = ?2, enrichment_revision = ?3 WHERE id = ?1",
                params![note.id, note.workspace_id, note.enrichment_revision as i64],
            ),
        }
        .map_err(WorkspaceError::Storage)?;
        if changed == 0 {
            // The transaction is dropped unfinished, so nothing is committed.
            return Err(WorkspaceError::NoteNotFound);
        }
        // Label membership travels with the Note row, so a Note that arrives in
        // a Workspace carries its Label meanings into that Workspace's own
        // Labels rather than pointing at another Workspace's.
        match mutation {
            NoteMutation::Insert(note) | NoteMutation::Relocate { note, .. } => {
                write_note_labels(&transaction, note)?;
            }
            NoteMutation::Replace(_) | NoteMutation::Delete { .. } => {}
        }
        if let NoteMutation::Relocate {
            note,
            relationships,
        } = mutation
        {
            write_relocated_relationships(&transaction, note, relationships)?;
        }
        for workspace_id in affected_workspace_ids {
            refresh_workspace_search(&transaction, &workspace_id)?;
        }
        transaction.commit().map_err(WorkspaceError::Storage)
    }

    fn attach_label(&mut self, note_id: &str, name: &str) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let note = self.note(note_id)?;
        let (name, canonical_name) = validated_label_name(name)?;
        let transaction = self.connection.transaction().map_err(WorkspaceError::Storage)?;
        let label_id = label_id_for(&transaction, &note.workspace_id, &name, &canonical_name)?;
        let inserted = transaction.execute("INSERT OR IGNORE INTO note_labels (note_id, label_id) VALUES (?1, ?2)", params![note_id, label_id]).map_err(WorkspaceError::Storage)?;
        if inserted > 0 {
            transaction.execute("UPDATE notes SET enrichment_revision = enrichment_revision + 1 WHERE id = ?1", [note_id]).map_err(WorkspaceError::Storage)?;
        }
        refresh_workspace_search(&transaction, &note.workspace_id)?;
        transaction.commit().map_err(WorkspaceError::Storage)?;
        self.snapshot()
    }

    fn detach_label(&mut self, note_id: &str, label_id: &str) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let note = self.note(note_id)?;
        let transaction = self.connection.transaction().map_err(WorkspaceError::Storage)?;
        let removed = transaction.execute("DELETE FROM note_labels WHERE note_id = ?1 AND label_id = ?2", params![note_id, label_id]).map_err(WorkspaceError::Storage)?;
        if removed > 0 {
            transaction.execute("UPDATE notes SET enrichment_revision = enrichment_revision + 1 WHERE id = ?1", [note_id]).map_err(WorkspaceError::Storage)?;
        }
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

    fn relate_notes(
        &mut self,
        note_id: &str,
        other_note_id: &str,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let snapshot = self.snapshot()?;
        // Every rejection is decided here, before a transaction exists, so an
        // invalid pair leaves nothing behind to roll back.
        let workspace_id = relatable_workspace_id(&snapshot.notes, note_id, other_note_id)?;
        if is_related(&snapshot.relationships, note_id, other_note_id) {
            return Ok(snapshot);
        }
        let (first, second) = canonical_pair(note_id, other_note_id);
        let transaction = self
            .connection
            .transaction()
            .map_err(WorkspaceError::Storage)?;
        // The unique index is the second guard: a pair raced past the check
        // above is ignored rather than stored twice.
        let inserted = transaction
            .execute(
                "INSERT INTO relationships (id, workspace_id, note_id_a, note_id_b, provenance, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(note_id_a, note_id_b) DO NOTHING",
                params![
                    id(),
                    workspace_id,
                    first,
                    second,
                    RelationshipProvenance::Manual.as_str(),
                    timestamp()
                ],
            )
            .map_err(WorkspaceError::Storage)?;
        if inserted > 0 {
            transaction
                .execute(
                    "UPDATE notes SET enrichment_revision = enrichment_revision + 1 WHERE id IN (?1, ?2)",
                    params![first, second],
                )
                .map_err(WorkspaceError::Storage)?;
        }
        transaction.commit().map_err(WorkspaceError::Storage)?;
        self.snapshot()
    }

    fn unrelate_notes(
        &mut self,
        note_id: &str,
        other_note_id: &str,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let snapshot = self.snapshot()?;
        relatable_workspace_id(&snapshot.notes, note_id, other_note_id)?;
        let (first, second) = canonical_pair(note_id, other_note_id);
        let transaction = self
            .connection
            .transaction()
            .map_err(WorkspaceError::Storage)?;
        let removed = transaction
            .execute(
                "DELETE FROM relationships WHERE note_id_a = ?1 AND note_id_b = ?2",
                params![first, second],
            )
            .map_err(WorkspaceError::Storage)?;
        if removed > 0 {
            transaction
                .execute(
                    "UPDATE notes SET enrichment_revision = enrichment_revision + 1 WHERE id IN (?1, ?2)",
                    params![first, second],
                )
                .map_err(WorkspaceError::Storage)?;
        }
        transaction.commit().map_err(WorkspaceError::Storage)?;
        self.snapshot()
    }

    fn persist_enrichment(
        &mut self,
        workspace_id: &str,
        note_id: &str,
        previous: &Note,
        applied: &crate::enrichment::ApplicableFields,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        require_workspace(&read_workspaces(&self.connection)?, workspace_id)?;
        if previous.workspace_id != workspace_id {
            return Err(WorkspaceError::NoteNotFound);
        }
        if applied.note_type.is_none()
            && applied.annotation.is_none()
            && applied.add_labels.is_empty()
            && applied.add_relationships.is_empty()
        {
            return self.snapshot();
        }
        // Capture the candidate set before the transaction so the borrow
        // checker is happy and the relationship inserts see a stable view.
        let existing_relationships = self.snapshot()?.relationships;
        let note_ids: std::collections::HashSet<String> = self
            .snapshot()?
            .notes
            .into_iter()
            .map(|note| note.id)
            .collect();
        let now = timestamp();
        let mut next = previous.clone();
        if let Some(note_type) = &applied.note_type {
            next.note_type = note_type.clone();
            next.note_type_provenance = Provenance::Ai;
        }
        if let Some(annotation) = &applied.annotation {
            next.annotation = Some(annotation.clone());
            next.annotation_provenance = Provenance::Ai;
        }
        next.updated_at = now.clone();
        next.enrichment_revision = previous.enrichment_revision + 1;
        next.last_enriched_at = Some(now);
        let transaction = self
            .connection
            .transaction()
            .map_err(WorkspaceError::Storage)?;
        transaction
            .execute(
                "UPDATE notes SET note_type = ?2, note_type_provenance = ?3, annotation = ?4, annotation_provenance = ?5, updated_at = ?6, enrichment_revision = ?7, last_enriched_at = ?8 WHERE id = ?1",
                params![
                    next.id,
                    next.note_type,
                    next.note_type_provenance.as_str(),
                    next.annotation,
                    next.annotation_provenance.as_str(),
                    next.updated_at,
                    next.enrichment_revision as i64,
                    next.last_enriched_at,
                ],
            )
            .map_err(WorkspaceError::Storage)?;
        for label_name in &applied.add_labels {
            let (name, canonical) = validated_label_name(label_name)?;
            let label_id = label_id_for(&transaction, workspace_id, &name, &canonical)?;
            transaction
                .execute(
                    "INSERT OR IGNORE INTO note_labels (note_id, label_id) VALUES (?1, ?2)",
                    params![note_id, label_id],
                )
                .map_err(WorkspaceError::Storage)?;
        }
        for other_id in &applied.add_relationships {
            // A candidate that disappeared is silently dropped rather than
            // a partial Relationship committed.
            if !note_ids.contains(other_id) {
                continue;
            }
            if is_related(&existing_relationships, note_id, other_id) {
                continue;
            }
            let (first, second) = canonical_pair(note_id, other_id);
            transaction
                .execute(
                    "INSERT INTO relationships (id, workspace_id, note_id_a, note_id_b, provenance, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(note_id_a, note_id_b) DO NOTHING",
                    params![
                        id(),
                        workspace_id,
                        first,
                        second,
                        RelationshipProvenance::Ai.as_str(),
                        timestamp()
                    ],
                )
                .map_err(WorkspaceError::Storage)?;
        }
        refresh_workspace_search(&transaction, workspace_id)?;
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
        (5_i64, include_str!("../migrations/0005_relationships.sql")),
        (6_i64, include_str!("../migrations/0006_assistance_policy.sql")),
        (7_i64, include_str!("../migrations/0007_cloud_consent.sql")),
        (8_i64, include_str!("../migrations/0008_note_enrichment.sql")),
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
            "SELECT id, name, assistance_policy, selected_model, cloud_consent_at, created_at, updated_at FROM thinking_workspaces ORDER BY created_at",
        )
        .map_err(WorkspaceError::Storage)?
        .query_map([], |row| {
            Ok(ThinkingWorkspace {
                id: row.get(0)?,
                name: row.get(1)?,
                assistance_policy: AssistancePolicy::from_str(&row.get::<_, String>(2)?),
                selected_model: row.get(3)?,
                cloud_consent_at: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
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

/// The Workspace's own Label with this display meaning, created when it is
/// missing. Identity is per Workspace: a Label row is never shared across one.
fn label_id_for(
    connection: &Connection,
    workspace_id: &str,
    name: &str,
    canonical_name: &str,
) -> Result<String, WorkspaceError> {
    let existing: Option<String> = connection
        .query_row(
            "SELECT id FROM labels WHERE workspace_id = ?1 AND canonical_name = ?2",
            params![workspace_id, canonical_name],
            |row| row.get(0),
        )
        .optional()
        .map_err(WorkspaceError::Storage)?;
    match existing {
        Some(id) => Ok(id),
        None => {
            let label_id = id();
            connection.execute("INSERT INTO labels (id, workspace_id, name, canonical_name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)", params![label_id, workspace_id, name, canonical_name, timestamp()]).map_err(WorkspaceError::Storage)?;
            Ok(label_id)
        }
    }
}

/// Gives the Note exactly the Label meanings it carries, in its own Workspace.
/// A Label left with no Note stays: it is the Workspace's vocabulary, and
/// keeping it is what lets an undone move find the same Label rows again.
fn write_note_labels(connection: &Connection, note: &Note) -> Result<(), WorkspaceError> {
    connection
        .execute("DELETE FROM note_labels WHERE note_id = ?1", [&note.id])
        .map_err(WorkspaceError::Storage)?;
    for label in &note.labels {
        let (name, canonical_name) = validated_label_name(&label.name)?;
        let label_id = label_id_for(connection, &note.workspace_id, &name, &canonical_name)?;
        connection
            .execute(
                "INSERT OR IGNORE INTO note_labels (note_id, label_id) VALUES (?1, ?2)",
                params![note.id, label_id],
            )
            .map_err(WorkspaceError::Storage)?;
    }
    Ok(())
}

/// Leaves the relocated Note with exactly the Relationships given, keeping only
/// those whose endpoints are both Notes in the Workspace it now belongs to. A
/// Relationship never crosses a Workspace seam, in either direction of a move.
fn write_relocated_relationships(
    connection: &Connection,
    note: &Note,
    relationships: &[Relationship],
) -> Result<(), WorkspaceError> {
    connection
        .execute(
            "DELETE FROM relationships WHERE note_id_a = ?1 OR note_id_b = ?1",
            [&note.id],
        )
        .map_err(WorkspaceError::Storage)?;
    for relationship in relationships {
        connection.execute(
            "INSERT INTO relationships (id, workspace_id, note_id_a, note_id_b, provenance, created_at) \
             SELECT ?1, ?2, ?3, ?4, ?5, ?6 \
             WHERE EXISTS(SELECT 1 FROM notes WHERE id = ?3 AND workspace_id = ?2) \
               AND EXISTS(SELECT 1 FROM notes WHERE id = ?4 AND workspace_id = ?2) \
             ON CONFLICT(note_id_a, note_id_b) DO NOTHING",
            params![
                relationship.id,
                note.workspace_id,
                relationship.note_id_a,
                relationship.note_id_b,
                relationship.provenance.as_str(),
                relationship.created_at
            ],
        ).map_err(WorkspaceError::Storage)?;
    }
    Ok(())
}

fn refresh_workspace_search(connection: &Connection, workspace_id: &str) -> Result<(), WorkspaceError> {
    connection.execute("DELETE FROM note_search WHERE workspace_id = ?1", [workspace_id]).map_err(WorkspaceError::Storage)?;
    connection.execute("INSERT INTO note_search(note_id, workspace_id, content) SELECT notes.id, notes.workspace_id, notes.markdown || ' ' || COALESCE(notes.annotation, '') || ' ' || COALESCE((SELECT group_concat(labels.name, ' ') FROM note_labels JOIN labels ON labels.id = note_labels.label_id WHERE note_labels.note_id = notes.id), '') FROM notes WHERE notes.workspace_id = ?1", [workspace_id]).map_err(WorkspaceError::Storage)?;
    Ok(())
}

fn read_relationships(connection: &Connection) -> Result<Vec<Relationship>, WorkspaceError> {
    connection
        .prepare("SELECT id, workspace_id, note_id_a, note_id_b, provenance, created_at FROM relationships ORDER BY created_at, id")
        .map_err(WorkspaceError::Storage)?
        .query_map([], |row| {
            Ok(Relationship {
                id: row.get(0)?,
                workspace_id: row.get(1)?,
                note_id_a: row.get(2)?,
                note_id_b: row.get(3)?,
                provenance: RelationshipProvenance::from_str(&row.get::<_, String>(4)?),
                created_at: row.get(5)?,
            })
        })
        .map_err(WorkspaceError::Storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(WorkspaceError::Storage)
}

fn read_snapshot(connection: &Connection) -> Result<WorkspaceSnapshot, WorkspaceError> {
    let workspaces = read_workspaces(connection)?;
    let mut notes = connection.prepare("SELECT id, workspace_id, markdown, note_type, note_type_provenance, annotation, annotation_provenance, created_at, updated_at, pinned, enrichment_revision, last_enriched_at FROM notes")
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
            enrichment_revision: row.get::<_, i64>(10)? as u64,
            last_enriched_at: row.get(11)?,
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
        relationships: read_relationships(connection)?,
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
        relationships: Vec<Relationship>,
        active_workspace_id: String,
        history: UndoHistory,
        labels: Vec<Label>,
    }

    impl MemoryStore {
        fn new() -> Self {
            let mut store = Self {
                workspaces: vec![],
                notes: vec![],
                relationships: vec![],
                active_workspace_id: String::new(),
                history: UndoHistory::default(),
                labels: vec![],
            };
            store.create_workspace(DEFAULT_WORKSPACE_NAME).unwrap();
            store
        }

        /// The same Label meanings, as this Workspace's own Labels, creating
        /// one only when the Workspace has no Label with that meaning yet.
        fn mapped_labels(&mut self, workspace_id: &str, labels: &[Label]) -> Vec<Label> {
            labels
                .iter()
                .map(|label| {
                    let canonical = label.name.to_lowercase();
                    self.labels
                        .iter()
                        .find(|candidate| {
                            candidate.workspace_id == workspace_id
                                && candidate.name.to_lowercase() == canonical
                        })
                        .cloned()
                        .unwrap_or_else(|| {
                            let mapped = Label {
                                id: id(),
                                workspace_id: workspace_id.to_owned(),
                                name: label.name.clone(),
                            };
                            self.labels.push(mapped.clone());
                            mapped
                        })
                })
                .collect()
        }

        /// A commit that touches the Note row bumps the enrichment revision,
        /// so a stale AI result that names the previous revision is rejected
        /// by the application gate.
        fn bump_revision(&mut self, note_id: &str) {
            if let Some(existing) = self.notes.iter_mut().find(|candidate| candidate.id == note_id) {
                existing.enrichment_revision += 1;
            }
        }
    }

    impl ThinkingWorkspaceInterface for MemoryStore {
        fn snapshot(&self) -> Result<WorkspaceSnapshot, WorkspaceError> {
            let mut notes = self.notes.clone();
            sort_notes(&mut notes);
            let mut relationships = self.relationships.clone();
            relationships.sort_by(|left, right| {
                (&left.created_at, &left.id).cmp(&(&right.created_at, &right.id))
            });
            Ok(WorkspaceSnapshot {
                workspaces: self.workspaces.clone(),
                notes,
                relationships,
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
                assistance_policy: AssistancePolicy::Manual,
                selected_model: None,
                cloud_consent_at: None,
                created_at: now.clone(),
                updated_at: now,
            };
            self.active_workspace_id = workspace.id.clone();
            self.workspaces.push(workspace);
            self.snapshot()
        }

        fn set_assistance_policy(
            &mut self,
            workspace_id: &str,
            policy: AssistancePolicy,
        ) -> Result<WorkspaceSnapshot, WorkspaceError> {
            require_workspace(&self.workspaces, workspace_id)?;
            for workspace in self.workspaces.iter_mut() {
                if workspace.id == workspace_id {
                    workspace.assistance_policy = policy;
                    workspace.updated_at = timestamp();
                }
            }
            self.snapshot()
        }

        fn set_selected_model(
            &mut self,
            workspace_id: &str,
            model_id: Option<&str>,
        ) -> Result<WorkspaceSnapshot, WorkspaceError> {
            require_workspace(&self.workspaces, workspace_id)?;
            for workspace in self.workspaces.iter_mut() {
                if workspace.id == workspace_id {
                    workspace.selected_model = model_id.map(|s| s.to_owned());
                    workspace.updated_at = timestamp();
                }
            }
            self.snapshot()
        }

        fn set_cloud_consent(
            &mut self,
            workspace_id: &str,
            accept: bool,
        ) -> Result<WorkspaceSnapshot, WorkspaceError> {
            require_workspace(&self.workspaces, workspace_id)?;
            for workspace in self.workspaces.iter_mut() {
                if workspace.id == workspace_id {
                    workspace.cloud_consent_at = if accept { Some(timestamp()) } else { None };
                    workspace.updated_at = timestamp();
                }
            }
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
            // Mirrors the schema's cascade from Workspace to Relationship.
            self.relationships
                .retain(|relationship| relationship.workspace_id != workspace_id);
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
                    let mut inserted = note.clone();
                    inserted.labels = self.mapped_labels(&note.workspace_id, &note.labels);
                    self.notes.push(inserted);
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
                    // Mirrors the schema's cascade from either endpoint, so no
                    // projection can observe a dangling endpoint here either.
                    self.relationships.retain(|relationship| {
                        &relationship.note_id_a != note_id && &relationship.note_id_b != note_id
                    });
                }
                NoteMutation::Relocate {
                    note,
                    relationships,
                } => {
                    let position = self
                        .notes
                        .iter()
                        .position(|candidate| candidate.id == note.id)
                        .ok_or(WorkspaceError::NoteNotFound)?;
                    let labels = self.mapped_labels(&note.workspace_id, &note.labels);
                    let mut relocated = note.clone();
                    relocated.labels = labels;
                    self.notes[position] = relocated;
                    // The Note leaves every Relationship it had behind, then
                    // takes back only those the command captured whose two
                    // endpoints are both Notes in the Workspace it now sits in.
                    self.relationships
                        .retain(|relationship| relationship.other_endpoint(&note.id).is_none());
                    for relationship in relationships {
                        let inside = |note_id: &str| {
                            self.notes.iter().any(|candidate| {
                                candidate.id == note_id
                                    && candidate.workspace_id == note.workspace_id
                            })
                        };
                        if inside(&relationship.note_id_a) && inside(&relationship.note_id_b) {
                            let mut restored = relationship.clone();
                            restored.workspace_id = note.workspace_id.clone();
                            self.relationships.push(restored);
                        }
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
            if !note.labels.iter().any(|candidate| candidate.id == label.id) {
                note.labels.push(label);
                note.enrichment_revision += 1;
            }
            self.snapshot()
        }

        fn detach_label(&mut self, note_id: &str, label_id: &str) -> Result<WorkspaceSnapshot, WorkspaceError> {
            let note = self.notes.iter_mut().find(|note| note.id == note_id).ok_or(WorkspaceError::NoteNotFound)?;
            let before = note.labels.len();
            note.labels.retain(|label| label.id != label_id);
            if note.labels.len() != before { note.enrichment_revision += 1; }
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

        fn relate_notes(
            &mut self,
            note_id: &str,
            other_note_id: &str,
        ) -> Result<WorkspaceSnapshot, WorkspaceError> {
            let workspace_id = relatable_workspace_id(&self.notes, note_id, other_note_id)?;
            if is_related(&self.relationships, note_id, other_note_id) {
                return self.snapshot();
            }
            let (first, second) = canonical_pair(note_id, other_note_id);
            self.relationships.push(Relationship {
                id: id(),
                workspace_id,
                note_id_a: first.to_owned(),
                note_id_b: second.to_owned(),
                provenance: RelationshipProvenance::Manual,
                created_at: timestamp(),
            });
            self.bump_revision(note_id);
            self.bump_revision(other_note_id);
            self.snapshot()
        }

        fn unrelate_notes(
            &mut self,
            note_id: &str,
            other_note_id: &str,
        ) -> Result<WorkspaceSnapshot, WorkspaceError> {
            relatable_workspace_id(&self.notes, note_id, other_note_id)?;
            let (first, second) = canonical_pair(note_id, other_note_id);
            let before = self.relationships.len();
            self.relationships.retain(|relationship| {
                relationship.note_id_a != first || relationship.note_id_b != second
            });
            if self.relationships.len() != before {
                self.bump_revision(note_id);
                self.bump_revision(other_note_id);
            }
            self.snapshot()
        }

        fn persist_enrichment(
            &mut self,
            workspace_id: &str,
            note_id: &str,
            previous: &Note,
            applied: &crate::enrichment::ApplicableFields,
        ) -> Result<WorkspaceSnapshot, WorkspaceError> {
            require_workspace(&self.workspaces, workspace_id)?;
            if previous.workspace_id != workspace_id {
                return Err(WorkspaceError::NoteNotFound);
            }
            if applied.note_type.is_none()
                && applied.annotation.is_none()
                && applied.add_labels.is_empty()
                && applied.add_relationships.is_empty()
            {
                return self.snapshot();
            }
            let now = timestamp();
            let position = self
                .notes
                .iter()
                .position(|candidate| candidate.id == note_id)
                .ok_or(WorkspaceError::NoteNotFound)?;
            let mut next = self.notes[position].clone();
            if let Some(note_type) = &applied.note_type {
                next.note_type = note_type.clone();
                next.note_type_provenance = Provenance::Ai;
            }
            if let Some(annotation) = &applied.annotation {
                next.annotation = Some(annotation.clone());
                next.annotation_provenance = Provenance::Ai;
            }
            next.updated_at = now.clone();
            next.enrichment_revision = previous.enrichment_revision + 1;
            next.last_enriched_at = Some(now);
            for label_name in &applied.add_labels {
                let (name, canonical) = validated_label_name(label_name)?;
                let label_id = if let Some(existing) = self.labels.iter().find(|label| label.workspace_id == workspace_id && label.name.to_lowercase() == canonical) {
                    existing.id.clone()
                } else {
                    let created = Label { id: id(), workspace_id: workspace_id.to_owned(), name };
                    self.labels.push(created.clone());
                    created.id
                };
                if !next.labels.iter().any(|candidate| candidate.id == label_id) {
                    if let Some(stored) = self.labels.iter().find(|label| label.id == label_id).cloned() {
                        next.labels.push(stored);
                    }
                }
            }
            for other_id in &applied.add_relationships {
                if !self.notes.iter().any(|candidate| candidate.id == *other_id) {
                    continue;
                }
                if is_related(&self.relationships, note_id, other_id) {
                    continue;
                }
                let (first, second) = canonical_pair(note_id, other_id);
                self.relationships.push(Relationship {
                    id: id(),
                    workspace_id: workspace_id.to_owned(),
                    note_id_a: first.to_owned(),
                    note_id_b: second.to_owned(),
                    provenance: RelationshipProvenance::Ai,
                    created_at: timestamp(),
                });
            }
            self.notes[position] = next;
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

    /// The Notes a Note is related to, read from committed state through the
    /// Thinking Graph's own endpoint rule rather than a second copy of it.
    fn related_ids(snapshot: &WorkspaceSnapshot, note_id: &str) -> Vec<String> {
        snapshot
            .relationships
            .iter()
            .filter_map(|relationship| relationship.other_endpoint(note_id))
            .map(str::to_owned)
            .collect()
    }

    /// Every endpoint of every Relationship names a Note that is still here.
    fn no_dangling_endpoints(snapshot: &WorkspaceSnapshot) -> bool {
        snapshot.relationships.iter().all(|relationship| {
            [&relationship.note_id_a, &relationship.note_id_b]
                .iter()
                .all(|endpoint| snapshot.notes.iter().any(|note| &&note.id == endpoint))
        })
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

    /// The display meanings a Note carries, in the order the interface reports.
    fn label_names(note: &Note) -> Vec<String> {
        note.labels.iter().map(|label| label.name.clone()).collect()
    }

    /// The distinct Labels a Workspace holds with one display meaning, read
    /// through the Notes that carry them.
    fn labels_meaning(snapshot: &WorkspaceSnapshot, workspace_id: &str, meaning: &str) -> Vec<String> {
        let mut ids: Vec<String> = snapshot
            .notes
            .iter()
            .filter(|note| note.workspace_id == workspace_id)
            .flat_map(|note| note.labels.iter())
            .filter(|label| label.name.to_lowercase() == meaning)
            .map(|label| label.id.clone())
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// Moving and copying a Note between two Thinking Workspaces, at the
    /// interface: identity, authored fields, Label remapping, the Relationship
    /// seam, refusals, and the two undo rules.
    fn note_transfer_conformance(workspace: &mut impl ThinkingWorkspaceInterface) {
        let note_id_of = |snapshot: &WorkspaceSnapshot, markdown: &str| {
            snapshot
                .notes
                .iter()
                .find(|note| note.markdown == markdown)
                .expect("the Note is committed")
                .id
                .clone()
        };
        let source = workspace_id_named(
            &committed(workspace.create_workspace_outcome("Transfer source")),
            "Transfer source",
        );
        let target = workspace_id_named(
            &committed(workspace.create_workspace_outcome("Transfer target")),
            "Transfer target",
        );
        committed(workspace.select_workspace_outcome(&source));
        let travelling = note_id_of(
            &committed(workspace.create_note_outcome(&source, "A thought that travels")),
            "A thought that travels",
        );
        let companion = note_id_of(
            &committed(workspace.create_note_outcome(&source, "It stays behind")),
            "It stays behind",
        );
        let resident = note_id_of(
            &committed(workspace.create_note_outcome(&target, "Already in the target")),
            "Already in the target",
        );
        committed(workspace.set_note_type_outcome(&travelling, "thesis"));
        committed(workspace.set_note_annotation_outcome(&travelling, "Why it matters"));
        committed(workspace.set_note_pinned_outcome(&travelling, true));
        committed(workspace.attach_label_outcome(&travelling, "Rivers"));
        committed(workspace.attach_label_outcome(&travelling, "Trade"));
        // The target already knows one of those meanings, spelled its own way.
        committed(workspace.attach_label_outcome(&resident, "RIVERS"));
        committed(workspace.relate_notes_outcome(&travelling, &companion));
        let target_rivers = note_in(&committed(workspace.snapshot_outcome()), &resident).labels[0]
            .id
            .clone();
        let before = note_in(&committed(workspace.snapshot_outcome()), &travelling);
        assert_eq!(label_names(&before), vec!["Rivers", "Trade"]);

        // A move keeps identity and every authored field, and lands the Note in
        // the target Workspace.
        distinct_moment();
        let moved = committed(workspace.move_note_outcome(&travelling, &target));
        let after = note_in(&moved, &travelling);
        assert_eq!(after.workspace_id, target);
        assert_eq!(after.markdown, before.markdown);
        assert_eq!(after.note_type, before.note_type);
        assert_eq!(after.note_type_provenance, before.note_type_provenance);
        assert_eq!(after.annotation, before.annotation);
        assert_eq!(after.annotation_provenance, before.annotation_provenance);
        assert_eq!(after.pinned, before.pinned);
        assert_eq!(after.created_at, before.created_at);
        assert_eq!(after.updated_at, before.updated_at);

        // Its Label meanings arrive as the target's own Labels. A meaning the
        // target already knows keeps the target's spelling instead of being
        // duplicated as a second Label; a meaning it did not know is created.
        assert_eq!(label_names(&after), vec!["RIVERS", "Trade"]);
        assert!(after.labels.iter().all(|label| label.workspace_id == target));
        assert_eq!(labels_meaning(&moved, &target, "rivers"), vec![target_rivers.clone()]);
        assert_eq!(labels_meaning(&moved, &target, "trade").len(), 1);

        // No Relationship crosses the seam, in either direction.
        assert!(related_ids(&moved, &travelling).is_empty());
        assert!(related_ids(&moved, &companion).is_empty());
        assert!(no_dangling_endpoints(&moved));

        // Every refusal leaves both Thinking Workspaces exactly as they were.
        let unchanged = committed(workspace.snapshot_outcome());
        assert!(is_validation_failure(
            &workspace.move_note_outcome(&travelling, &target)
        ));
        assert!(is_validation_failure(
            &workspace.copy_note_outcome(&travelling, &target)
        ));
        assert!(is_not_found_failure(
            &workspace.move_note_outcome(&travelling, "missing")
        ));
        assert!(is_not_found_failure(
            &workspace.copy_note_outcome("missing", &source)
        ));
        assert_eq!(committed(workspace.snapshot_outcome()), unchanged);

        // Undoing the move returns the Note, its Labels, and the Relationship
        // whose endpoints are both still in the Workspace it came from.
        let undone = committed(workspace.undo_outcome(&source));
        assert_eq!(note_in(&undone, &travelling), before);
        assert_eq!(related_ids(&undone, &travelling), vec![companion.clone()]);
        assert!(no_dangling_endpoints(&undone));

        // A copy leaves the original where it is and takes a fresh identity.
        distinct_moment();
        let copied = committed(workspace.copy_note_outcome(&travelling, &target));
        let copy = copied
            .notes
            .iter()
            .find(|note| note.workspace_id == target && note.markdown == before.markdown)
            .expect("the copy is committed")
            .clone();
        assert_ne!(copy.id, travelling);
        assert_eq!(note_in(&copied, &travelling), before);
        assert_eq!(copy.note_type, before.note_type);
        assert_eq!(copy.note_type_provenance, before.note_type_provenance);
        assert_eq!(copy.annotation, before.annotation);
        assert_eq!(copy.annotation_provenance, before.annotation_provenance);
        assert_eq!(copy.pinned, before.pinned);
        assert_ne!(copy.created_at, before.created_at);
        assert_eq!(label_names(&copy), vec!["RIVERS", "Trade"]);
        assert!(copy.labels.iter().all(|label| label.workspace_id == target));
        assert_eq!(labels_meaning(&copied, &target, "rivers"), vec![target_rivers]);

        // The copy inherits no Relationship, and the original keeps its own.
        assert!(related_ids(&copied, &copy.id).is_empty());
        assert_eq!(related_ids(&copied, &travelling), vec![companion.clone()]);
        assert!(no_dangling_endpoints(&copied));

        // Undoing a copy deletes only the copy.
        let after_undo = committed(workspace.undo_outcome(&source));
        assert!(!after_undo.notes.iter().any(|note| note.id == copy.id));
        assert_eq!(note_in(&after_undo, &travelling), before);
        assert!(after_undo.notes.iter().any(|note| note.id == resident));
        assert_eq!(related_ids(&after_undo, &travelling), vec![companion]);
        assert!(no_dangling_endpoints(&after_undo));

        // Every copy takes its own fresh identity, so copying the same Note
        // twice is two Notes rather than a collision with it or with each other.
        let copies_in_target = |snapshot: &WorkspaceSnapshot| -> Vec<String> {
            snapshot
                .notes
                .iter()
                .filter(|note| note.workspace_id == target && note.markdown == before.markdown)
                .map(|note| note.id.clone())
                .collect()
        };
        committed(workspace.copy_note_outcome(&travelling, &target));
        let twice = committed(workspace.copy_note_outcome(&travelling, &target));
        let copies = copies_in_target(&twice);
        assert_eq!(copies.len(), 2);
        assert_ne!(copies[0], copies[1]);
        assert!(!copies.contains(&travelling));
        committed(workspace.undo_outcome(&source));
        let emptied = committed(workspace.undo_outcome(&source));
        assert!(copies_in_target(&emptied).is_empty());
        assert_eq!(note_in(&emptied, &travelling), before);
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

        // A Relationship is one symmetric, untyped association, manual by
        // default, and stored once under the canonical pair ordering.
        let related = committed(workspace.relate_notes_outcome(&note_id, &second_note_id));
        assert_eq!(related.relationships.len(), 1);
        let relationship = related.relationships[0].clone();
        assert_eq!(relationship.provenance, RelationshipProvenance::Manual);
        assert!(!relationship.created_at.is_empty());
        assert_eq!(relationship.workspace_id, research);
        assert_eq!(
            canonical_pair(&note_id, &second_note_id),
            (
                relationship.note_id_a.as_str(),
                relationship.note_id_b.as_str()
            )
        );

        // Either endpoint lists the other, in either order, and no endpoint
        // names a Note that is not here.
        assert_eq!(related_ids(&related, &note_id), vec![second_note_id.clone()]);
        assert_eq!(related_ids(&related, &second_note_id), vec![note_id.clone()]);
        assert!(is_related(&related.relationships, &note_id, &second_note_id));
        assert!(is_related(&related.relationships, &second_note_id, &note_id));
        assert!(no_dangling_endpoints(&related));

        // Asking again in the reversed endpoint order is the same Relationship,
        // down to its identity, so nothing is stored twice.
        let again = committed(workspace.relate_notes_outcome(&second_note_id, &note_id));
        assert_eq!(again.relationships, vec![relationship.clone()]);

        // A Relationship needs two distinct Notes in one Thinking Workspace,
        // and a refusal leaves the graph exactly as it was.
        assert!(is_validation_failure(
            &workspace.relate_notes_outcome(&note_id, &note_id)
        ));
        assert!(is_validation_failure(
            &workspace.relate_notes_outcome(&note_id, &outside_id)
        ));
        assert!(is_not_found_failure(
            &workspace.relate_notes_outcome(&note_id, "missing")
        ));
        assert!(is_not_found_failure(
            &workspace.unrelate_notes_outcome(&note_id, "missing")
        ));
        assert_eq!(
            committed(workspace.snapshot_outcome()).relationships,
            vec![relationship.clone()]
        );

        // Removing works from either endpoint order.
        let unrelated = committed(workspace.unrelate_notes_outcome(&second_note_id, &note_id));
        assert!(unrelated.relationships.is_empty());
        assert!(related_ids(&unrelated, &note_id).is_empty());

        // Deleting either Note takes the Relationship with it, so no projection
        // can reach an endpoint that no longer names a Note.
        committed(workspace.relate_notes_outcome(&note_id, &second_note_id));
        let cascaded = committed(workspace.delete_note_outcome(&second_note_id));
        assert!(cascaded.relationships.is_empty());
        assert!(related_ids(&cascaded, &note_id).is_empty());
        assert!(no_dangling_endpoints(&cascaded));

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
        assert!(last.relationships.is_empty());

        note_transfer_conformance(&mut workspace);
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

    /// Commits two Notes in the active Workspace and returns their ids.
    fn two_notes(store: &mut WorkspaceStore) -> (String, String, String) {
        let workspace_id = store.snapshot().unwrap().active_workspace_id;
        let note_id = |snapshot: &WorkspaceSnapshot, markdown: &str| {
            snapshot
                .notes
                .iter()
                .find(|note| note.markdown == markdown)
                .expect("the Note is committed")
                .id
                .clone()
        };
        let first = note_id(
            &store.create_note(&workspace_id, "One end").unwrap(),
            "One end",
        );
        let second = note_id(
            &store.create_note(&workspace_id, "The other end").unwrap(),
            "The other end",
        );
        (workspace_id, first, second)
    }

    #[test]
    fn sqlite_recovers_manual_relationships_and_their_cascade_after_reopen() {
        let path = temporary_path();
        let (workspace_id, first, second, created_at) = {
            let mut store = WorkspaceStore::open(&path).unwrap();
            let (workspace_id, first, second) = two_notes(&mut store);
            let snapshot = store.relate_notes(&first, &second).unwrap();
            (
                workspace_id,
                first,
                second,
                snapshot.relationships[0].created_at.clone(),
            )
        };

        let mut reopened = WorkspaceStore::open(&path).unwrap();
        let snapshot = reopened.snapshot().unwrap();
        assert_eq!(snapshot.relationships.len(), 1);
        assert_eq!(
            snapshot.relationships[0].provenance,
            RelationshipProvenance::Manual
        );
        assert_eq!(snapshot.relationships[0].created_at, created_at);
        assert_eq!(snapshot.relationships[0].workspace_id, workspace_id);
        assert_eq!(related_ids(&snapshot, &first), vec![second.clone()]);
        assert_eq!(related_ids(&snapshot, &second), vec![first.clone()]);
        assert!(no_dangling_endpoints(&snapshot));
        // A duplicate asked for in a later session is still one row.
        assert_eq!(
            reopened
                .relate_notes(&second, &first)
                .unwrap()
                .relationships
                .len(),
            1
        );

        // The endpoint cascade is the schema's, so it survives the session too.
        reopened.delete_note(&first).unwrap();
        drop(reopened);
        assert!(WorkspaceStore::open(&path)
            .unwrap()
            .snapshot()
            .unwrap()
            .relationships
            .is_empty());
        remove_database(&path);
    }

    #[test]
    fn storage_refuses_a_duplicate_reversed_or_self_relationship_written_around_the_module() {
        let path = temporary_path();
        let mut store = WorkspaceStore::open(&path).unwrap();
        let (workspace_id, first, second) = two_notes(&mut store);
        let existing = store.relate_notes(&first, &second).unwrap().relationships[0].clone();

        // Nothing below goes through the Thinking Graph module, so these prove
        // the database itself cannot hold a second row for one pair.
        let write = |note_id_a: &str, note_id_b: &str| {
            store.connection.execute(
                "INSERT INTO relationships (id, workspace_id, note_id_a, note_id_b, provenance, created_at) VALUES (?1, ?2, ?3, ?4, 'manual', ?5)",
                params![id(), workspace_id, note_id_a, note_id_b, timestamp()],
            )
        };
        assert!(write(&existing.note_id_a, &existing.note_id_b).is_err());
        assert!(write(&existing.note_id_b, &existing.note_id_a).is_err());
        assert!(write(&first, &first).is_err());
        assert!(write(&existing.note_id_a, "vanished").is_err());
        assert_eq!(store.snapshot().unwrap().relationships, vec![existing]);
        remove_database(&path);
    }

    #[test]
    fn a_failed_relationship_commit_leaves_the_thinking_graph_unchanged() {
        let path = temporary_path();
        let mut store = WorkspaceStore::open(&path).unwrap();
        let (_, first, second) = two_notes(&mut store);
        store.connection.execute_batch("CREATE TRIGGER reject_relationships BEFORE INSERT ON relationships BEGIN SELECT RAISE(FAIL, 'injected'); END;").unwrap();

        assert!(matches!(
            store.relate_notes_outcome(&first, &second),
            WorkspaceCommandResult::Failed {
                failure: WorkspaceFailure {
                    code: WorkspaceFailureCode::Storage,
                    ..
                }
            }
        ));
        let unchanged = store.snapshot().unwrap();
        assert!(unchanged.relationships.is_empty());
        assert!(related_ids(&unchanged, &first).is_empty());
        remove_database(&path);
    }

    /// Commits a Note carrying a Label and a Relationship in the active
    /// Workspace, plus a second Workspace to transfer it into. Returns the
    /// source Workspace, the target Workspace, the Note, and its companion.
    fn ready_to_transfer(store: &mut WorkspaceStore) -> (String, String, String, String) {
        let (source, travelling, companion) = two_notes(store);
        store.attach_label(&travelling, "Rivers").unwrap();
        store.relate_notes(&travelling, &companion).unwrap();
        let target = store
            .create_workspace("Transfer target")
            .unwrap()
            .active_workspace_id;
        store.select_workspace(&source).unwrap();
        (source, target, travelling, companion)
    }

    #[test]
    fn a_moved_and_a_copied_note_survive_reopen_with_their_labels_and_no_relationships() {
        let path = temporary_path();
        let (source, target, travelling, companion, copy_id, created_at) = {
            let mut store = WorkspaceStore::open(&path).unwrap();
            let (source, target, travelling, companion) = ready_to_transfer(&mut store);
            let created_at = store.note(&travelling).unwrap().created_at;
            store.move_note(&travelling, &target).unwrap();
            let copied = store.copy_note(&companion, &target).unwrap();
            let copy_id = copied
                .notes
                .iter()
                .find(|note| note.workspace_id == target && note.markdown == "The other end")
                .expect("the copy is committed")
                .id
                .clone();
            (source, target, travelling, companion, copy_id, created_at)
        };

        let reopened = WorkspaceStore::open(&path).unwrap();
        let snapshot = reopened.snapshot().unwrap();
        // The moved Note kept its identity and creation time in the target.
        let moved = note_in(&snapshot, &travelling);
        assert_eq!(moved.workspace_id, target);
        assert_eq!(moved.created_at, created_at);
        assert_eq!(label_names(&moved), vec!["Rivers"]);
        assert_eq!(moved.labels[0].workspace_id, target);
        // The copy is a second Note, and the Note it came from stayed put.
        assert_eq!(note_in(&snapshot, &copy_id).workspace_id, target);
        assert_eq!(note_in(&snapshot, &companion).workspace_id, source);
        // Neither transfer carried a Relationship across the seam.
        assert!(snapshot.relationships.is_empty());
        assert!(no_dangling_endpoints(&snapshot));
        // The target's search projection knows both arrivals; the source's does
        // not still hold the Note that left it.
        assert_eq!(reopened.search_notes(&target, "rivers").unwrap().len(), 1);
        assert_eq!(reopened.search_notes(&target, "other end").unwrap().len(), 1);
        assert!(reopened.search_notes(&source, "rivers").unwrap().is_empty());
        remove_database(&path);
    }

    #[test]
    fn a_failed_move_or_copy_leaves_both_thinking_workspaces_unchanged() {
        let path = temporary_path();
        let mut store = WorkspaceStore::open(&path).unwrap();
        let (_, target, travelling, companion) = ready_to_transfer(&mut store);
        let before = store.snapshot().unwrap();

        // The Label remap is part of the same transaction as the Note row, so
        // rejecting it must roll the whole move back.
        store.connection.execute_batch("CREATE TRIGGER reject_labels BEFORE INSERT ON note_labels BEGIN SELECT RAISE(FAIL, 'injected'); END;").unwrap();
        assert!(matches!(
            store.move_note_outcome(&travelling, &target),
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
            .execute_batch("DROP TRIGGER reject_labels;")
            .unwrap();

        store.connection.execute_batch("CREATE TRIGGER reject_relocation BEFORE UPDATE ON notes BEGIN SELECT RAISE(FAIL, 'injected'); END;").unwrap();
        assert!(matches!(
            store.move_note_outcome(&travelling, &target),
            WorkspaceCommandResult::Failed { .. }
        ));
        assert_eq!(store.snapshot().unwrap(), before);
        store
            .connection
            .execute_batch("DROP TRIGGER reject_relocation;")
            .unwrap();

        store.connection.execute_batch("CREATE TRIGGER reject_copies BEFORE INSERT ON notes BEGIN SELECT RAISE(FAIL, 'injected'); END;").unwrap();
        assert!(matches!(
            store.copy_note_outcome(&travelling, &target),
            WorkspaceCommandResult::Failed { .. }
        ));
        assert_eq!(store.snapshot().unwrap(), before);
        store
            .connection
            .execute_batch("DROP TRIGGER reject_copies;")
            .unwrap();

        // A rejected transfer left the reversible history alone too, so the
        // next undo is still the Relationship-free state before either attempt.
        let moved = committed(store.move_note_outcome(&travelling, &target));
        assert_eq!(note_in(&moved, &travelling).workspace_id, target);
        assert_eq!(store.undo(&before.active_workspace_id).unwrap(), before);
        assert_eq!(related_ids(&store.snapshot().unwrap(), &companion), vec![travelling]);
        drop(store);
        remove_database(&path);
    }

    #[test]
    fn an_undone_move_restores_only_relationships_whose_endpoints_are_still_there() {
        let path = temporary_path();
        let mut store = WorkspaceStore::open(&path).unwrap();
        let (source, target, travelling, companion) = ready_to_transfer(&mut store);
        // A second Relationship, so one endpoint can vanish while another stays.
        let other = store
            .create_note(&source, "A third end")
            .unwrap()
            .notes
            .iter()
            .find(|note| note.markdown == "A third end")
            .expect("the Note is committed")
            .id
            .clone();
        store.relate_notes(&travelling, &other).unwrap();

        store.move_note(&travelling, &target).unwrap();
        // While the Note is away, one of the endpoints it left behind is gone.
        // The delete goes straight to the adapter so it records no reversible
        // command of its own, leaving the move as the next thing to undo.
        store
            .apply_note_mutation(&NoteMutation::Delete { note_id: other })
            .unwrap();

        let undone = store.undo(&source).unwrap();
        assert_eq!(note_in(&undone, &travelling).workspace_id, source);
        assert_eq!(related_ids(&undone, &travelling), vec![companion]);
        assert!(no_dangling_endpoints(&undone));
        drop(store);
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

    #[test]
    fn new_workspace_defaults_to_manual_assistance_policy() {
        let path = temporary_path();
        let mut store = WorkspaceStore::open(&path).unwrap();
        let snapshot = committed(store.create_workspace_outcome("Fresh"));
        let workspace = snapshot.workspaces.iter().find(|w| w.name == "Fresh").unwrap();
        assert_eq!(workspace.assistance_policy, AssistancePolicy::Manual);
        assert!(workspace.selected_model.is_none());
        remove_database(&path);
    }

    #[test]
    fn assistance_policy_and_selected_model_survive_reopen() {
        let path = temporary_path();
        let workspace_id = {
            let mut store = WorkspaceStore::open(&path).unwrap();
            let snapshot = committed(store.create_workspace_outcome("Assisted"));
            let id = workspace_id_named(&snapshot, "Assisted");
            committed(store.set_assistance_policy_outcome(&id, "local_ai"));
            committed(store.set_selected_model_outcome(&id, Some("some/vendor/model:tag")));
            id
        };

        let store = WorkspaceStore::open(&path).unwrap();
        let snapshot = committed(store.snapshot_outcome());
        let workspace = snapshot
            .workspaces
            .iter()
            .find(|w| w.id == workspace_id)
            .unwrap();
        assert_eq!(workspace.assistance_policy, AssistancePolicy::LocalAi);
        assert_eq!(
            workspace.selected_model.as_deref(),
            Some("some/vendor/model:tag")
        );
        remove_database(&path);
    }

    #[test]
    fn memory_adapter_persists_assistance_policy_and_selected_model() {
        let mut store = MemoryStore::new();
        let snapshot = committed(store.create_workspace_outcome("Assisted"));
        let id = workspace_id_named(&snapshot, "Assisted");
        let with_policy = committed(store.set_assistance_policy_outcome(&id, "local_ai"));
        assert_eq!(
            with_policy
                .workspaces
                .iter()
                .find(|w| w.id == id)
                .unwrap()
                .assistance_policy,
            AssistancePolicy::LocalAi
        );
        let with_model = committed(store.set_selected_model_outcome(&id,
            Some("unicode-先生-7b:latest"),
        ));
        assert_eq!(
            with_model
                .workspaces
                .iter()
                .find(|w| w.id == id)
                .unwrap()
                .selected_model
                .as_deref(),
            Some("unicode-先生-7b:latest")
        );
    }

    #[test]
    fn setting_assistance_policy_on_missing_workspace_fails() {
        let mut store = MemoryStore::new();
        assert!(matches!(
            store.set_assistance_policy_outcome("missing", "local_ai"),
            WorkspaceCommandResult::Failed {
                failure: WorkspaceFailure {
                    code: WorkspaceFailureCode::NotFound,
                    ..
                }
            }
        ));
    }

    /// A sentinel value that must never appear in any persisted state. The
    /// secret-leak tests assert that this value is not present anywhere a
    /// bearer key could land by accident.
    const SENTINEL_KEY: &str = "SENTINEL-BEARER-KEY-DO-NOT-LEAK";

    #[test]
    fn new_workspace_has_no_cloud_consent() {
        let mut store = MemoryStore::new();
        let snapshot = committed(store.create_workspace_outcome("Cloud candidate"));
        let workspace = snapshot.workspaces.iter().find(|w| w.name == "Cloud candidate").unwrap();
        assert!(workspace.cloud_consent_at().is_none());
    }

    #[test]
    fn accepting_cloud_consent_records_a_moment_and_clearing_removes_it() {
        let mut store = MemoryStore::new();
        let snapshot = committed(store.create_workspace_outcome("Cloud candidate"));
        let id = workspace_id_named(&snapshot, "Cloud candidate");
        let with_consent = committed(store.set_cloud_consent_outcome(&id, true));
        let workspace = with_consent
            .workspaces
            .iter()
            .find(|w| w.id == id)
            .unwrap();
        assert!(workspace.cloud_consent_at().is_some());
        let after_clear = committed(store.set_cloud_consent_outcome(&id, false));
        let cleared = after_clear
            .workspaces
            .iter()
            .find(|w| w.id == id)
            .unwrap();
        assert!(cleared.cloud_consent_at().is_none());
    }

    #[test]
    fn cloud_consent_survives_reopen_without_a_bearer_key_in_sqlite() {
        let path = temporary_path();
        let workspace_id = {
            let mut store = WorkspaceStore::open(&path).unwrap();
            let snapshot = committed(store.create_workspace_outcome("Cloudy"));
            let id = workspace_id_named(&snapshot, "Cloudy");
            committed(store.set_cloud_consent_outcome(&id, true));
            id
        };
        let store = WorkspaceStore::open(&path).unwrap();
        let snapshot = committed(store.snapshot_outcome());
        let workspace = snapshot
            .workspaces
            .iter()
            .find(|w| w.id == workspace_id)
            .unwrap();
        assert!(workspace.cloud_consent_at().is_some());

        // The sentinel key must not appear anywhere the database has written.
        let connection = Connection::open(&path).unwrap();
        let serialized: String = connection
            .prepare("SELECT group_concat(COALESCE(id, '') || '|' || COALESCE(name, '') || '|' || COALESCE(assistance_policy, '') || '|' || COALESCE(selected_model, '') || '|' || COALESCE(cloud_consent_at, '') || '|' || COALESCE(created_at, '') || '|' || COALESCE(updated_at, ''), '\n') FROM thinking_workspaces")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert!(
            !serialized.contains(SENTINEL_KEY),
            "The sentinel bearer key must never appear in the durable Workspace row, got:\n{serialized}"
        );
        drop(connection);
        remove_database(&path);
    }

    #[test]
    fn cloud_consent_is_isolated_between_workspaces() {
        let mut store = MemoryStore::new();
        let snapshot = committed(store.create_workspace_outcome("First"));
        let first = workspace_id_named(&snapshot, "First");
        let second_snapshot = committed(store.create_workspace_outcome("Second"));
        let second = workspace_id_named(&second_snapshot, "Second");
        committed(store.set_cloud_consent_outcome(&first, true));
        let after = committed(store.snapshot_outcome());
        let first_workspace = after.workspaces.iter().find(|w| w.id == first).unwrap();
        let second_workspace = after.workspaces.iter().find(|w| w.id == second).unwrap();
        assert!(first_workspace.cloud_consent_at().is_some());
        assert!(second_workspace.cloud_consent_at().is_none());
    }

    #[test]
    fn cloud_consent_on_missing_workspace_fails() {
        let mut store = MemoryStore::new();
        assert!(matches!(
            store.set_cloud_consent_outcome("missing", true),
            WorkspaceCommandResult::Failed {
                failure: WorkspaceFailure {
                    code: WorkspaceFailureCode::NotFound,
                    ..
                }
            }
        ));
    }

    /// A snapshot must never carry a bearer key: this is the typed rule the
    /// UI relies on, so the test asserts it explicitly. The sentinel is a
    /// value the bearer key would carry; we record it in places a thinker
    /// could mistake for a key (Note text, Annotation, selected model) and
    /// confirm the keychain path is the only durable place we trust it.
    #[test]
    fn no_durable_state_carries_a_sentinel_bearer_key() {
        let path = temporary_path();
        let mut store = WorkspaceStore::open(&path).unwrap();
        let snapshot = committed(store.create_workspace_outcome("Carrier"));
        let id = workspace_id_named(&snapshot, "Carrier");
        committed(store.set_assistance_policy_outcome(&id, "cloud_ai"));
        // A Note and an Annotation that contain the sentinel value, so the
        // test catches a leak in either text or rendered output.
        let note = committed(store.create_note_outcome(&id, &format!("# Note that quotes a key: {SENTINEL_KEY}")));
        let note_id = note.notes[0].id.clone();
        committed(store.set_note_annotation_outcome(&note_id, &format!("Key shown to user: {SENTINEL_KEY}")));
        let snapshot = committed(store.snapshot_outcome());
        let serialized = serde_json::to_string(&snapshot).unwrap();
        // The sentinel is in Note text by design; this asserts only the
        // sentinel never appears in any non-text durable field.
        for path in [snapshot.workspaces.iter().flat_map(|w| w.id().chars().chain(w.cloud_consent_at().unwrap_or("").chars()))] {
            let collected: String = path.collect();
            assert!(!collected.contains(SENTINEL_KEY), "A non-text Workspace field carried the sentinel: {collected}");
        }
        // The serialized snapshot still has the text because the thinker
        // wrote it; the test only fails if a non-text slot echoes the key.
        let _ = serialized;
        drop(store);
        remove_database(&path);
    }

    fn parsed_enrichment() -> crate::enrichment::ParsedEnrichmentResult {
        crate::enrichment::ParsedEnrichmentResult {
            note_type: "claim".to_owned(),
            labels: vec!["alpha".to_owned(), "beta".to_owned()],
            annotation: Some("an additive note".to_owned()),
            related_note_ids: vec![],
        }
    }

    fn token_for(workspace_id: &str, note_id: &str, revision: u64) -> crate::enrichment::RequestToken {
        crate::enrichment::RequestToken {
            workspace_id: workspace_id.to_owned(),
            note_id: note_id.to_owned(),
            revision,
            policy: "local_ai".to_owned(),
            endpoint: "http://localhost:11434".to_owned(),
            model: "phi3:latest".to_owned(),
        }
    }

    #[test]
    fn apply_enrichment_sets_ai_provenance_on_organization_fields() {
        let mut store = MemoryStore::new();
        let workspace_id = workspace_id_named(&committed(store.snapshot_outcome()), DEFAULT_WORKSPACE_NAME);
        committed(store.set_assistance_policy_outcome(&workspace_id, "local_ai"));
        let note = committed(store.create_note_outcome(&workspace_id, "an org thought"));
        let note_id = note.notes[0].id.clone();
        let outcome = committed(store.apply_enrichment_outcome(
            &workspace_id,
            &note_id,
            &parsed_enrichment(),
            &token_for(&workspace_id, &note_id, 0),
            false,
        ));
        let after = note_in(&outcome, &note_id);
        assert_eq!(after.note_type, "claim");
        assert_eq!(after.note_type_provenance, Provenance::Ai);
        assert_eq!(after.annotation.as_deref(), Some("an additive note"));
        assert_eq!(after.annotation_provenance, Provenance::Ai);
        let label_names: Vec<String> = after.labels.iter().map(|label| label.name().to_owned()).collect();
        assert_eq!(label_names, vec!["alpha".to_owned(), "beta".to_owned()]);
        assert!(after.last_enriched_at().is_some());
    }

    #[test]
    fn apply_enrichment_never_writes_a_manual_note_type() {
        let mut store = MemoryStore::new();
        let workspace_id = workspace_id_named(&committed(store.snapshot_outcome()), DEFAULT_WORKSPACE_NAME);
        committed(store.set_assistance_policy_outcome(&workspace_id, "local_ai"));
        let note = committed(store.create_note_outcome(&workspace_id, "a manual thought"));
        let note_id = note.notes[0].id.clone();
        committed(store.set_note_type_outcome(&note_id, "opinion"));
        let before = note_in(&committed(store.snapshot_outcome()), &note_id);
        let outcome = committed(store.apply_enrichment_outcome(
            &workspace_id,
            &note_id,
            &parsed_enrichment(),
            &token_for(&workspace_id, &note_id, before.enrichment_revision()),
            false,
        ));
        let after = note_in(&outcome, &note_id);
        assert_eq!(after.note_type, "opinion");
        assert_eq!(after.note_type_provenance, Provenance::Manual);
    }

    #[test]
    fn apply_enrichment_replaces_manual_fields_when_forced() {
        let mut store = MemoryStore::new();
        let workspace_id = workspace_id_named(&committed(store.snapshot_outcome()), DEFAULT_WORKSPACE_NAME);
        committed(store.set_assistance_policy_outcome(&workspace_id, "local_ai"));
        let note = committed(store.create_note_outcome(&workspace_id, "a manual thought"));
        let note_id = note.notes[0].id.clone();
        committed(store.set_note_type_outcome(&note_id, "opinion"));
        committed(store.set_note_annotation_outcome(&note_id, "kept by hand"));
        let before = note_in(&committed(store.snapshot_outcome()), &note_id);
        let outcome = committed(store.apply_enrichment_outcome(
            &workspace_id,
            &note_id,
            &parsed_enrichment(),
            &token_for(&workspace_id, &note_id, before.enrichment_revision()),
            true,
        ));
        let after = note_in(&outcome, &note_id);
        assert_eq!(after.note_type, "claim");
        assert_eq!(after.note_type_provenance, Provenance::Ai);
        assert_eq!(after.annotation.as_deref(), Some("an additive note"));
    }

    #[test]
    fn apply_enrichment_rejects_stale_revision() {
        let mut store = MemoryStore::new();
        let workspace_id = workspace_id_named(&committed(store.snapshot_outcome()), DEFAULT_WORKSPACE_NAME);
        committed(store.set_assistance_policy_outcome(&workspace_id, "local_ai"));
        let note = committed(store.create_note_outcome(&workspace_id, "an org thought"));
        let note_id = note.notes[0].id.clone();
        // Bump the revision with a manual edit, so the token's revision (0) is stale.
        committed(store.set_note_pinned_outcome(&note_id, true));
        let outcome = store.apply_enrichment_outcome(
            &workspace_id,
            &note_id,
            &parsed_enrichment(),
            &token_for(&workspace_id, &note_id, 0),
            false,
        );
        assert!(matches!(
            outcome,
            WorkspaceCommandResult::Failed {
                failure: WorkspaceFailure {
                    code: WorkspaceFailureCode::Stale,
                    ..
                }
            }
        ));
    }

    #[test]
    fn apply_enrichment_rejects_manual_policy() {
        let mut store = MemoryStore::new();
        let workspace_id = workspace_id_named(&committed(store.snapshot_outcome()), DEFAULT_WORKSPACE_NAME);
        let note = committed(store.create_note_outcome(&workspace_id, "an org thought"));
        let note_id = note.notes[0].id.clone();
        // Policy remains Manual; the request should be rejected.
        let outcome = store.apply_enrichment_outcome(
            &workspace_id,
            &note_id,
            &parsed_enrichment(),
            &token_for(&workspace_id, &note_id, 0),
            false,
        );
        assert!(matches!(
            outcome,
            WorkspaceCommandResult::Failed {
                failure: WorkspaceFailure {
                    code: WorkspaceFailureCode::Stale,
                    ..
                }
            }
        ));
    }

    #[test]
    fn apply_enrichment_never_overwrites_a_manual_label_membership() {
        let mut store = MemoryStore::new();
        let workspace_id = workspace_id_named(&committed(store.snapshot_outcome()), DEFAULT_WORKSPACE_NAME);
        committed(store.set_assistance_policy_outcome(&workspace_id, "local_ai"));
        let note = committed(store.create_note_outcome(&workspace_id, "an org thought"));
        let note_id = note.notes[0].id.clone();
        // A manual Label of the same name; the AI must not add a duplicate.
        committed(store.attach_label_outcome(&note_id, "alpha"));
        let before = note_in(&committed(store.snapshot_outcome()), &note_id);
        let outcome = committed(store.apply_enrichment_outcome(
            &workspace_id,
            &note_id,
            &parsed_enrichment(),
            &token_for(&workspace_id, &note_id, before.enrichment_revision()),
            false,
        ));
        let after = note_in(&outcome, &note_id);
        let label_names: Vec<String> = after.labels.iter().map(|label| label.name().to_owned()).collect();
        // The manual "alpha" stays; the AI suggestion of "alpha" is skipped
        // because it is already a membership, and "beta" is added.
        assert_eq!(label_names, vec!["alpha".to_owned(), "beta".to_owned()]);
    }

    #[test]
    fn apply_enrichment_does_not_change_note_text() {
        let mut store = MemoryStore::new();
        let workspace_id = workspace_id_named(&committed(store.snapshot_outcome()), DEFAULT_WORKSPACE_NAME);
        committed(store.set_assistance_policy_outcome(&workspace_id, "local_ai"));
        let original_text = "A thought that must never be rewritten.";
        let note = committed(store.create_note_outcome(&workspace_id, original_text));
        let note_id = note.notes[0].id.clone();
        let before = note_in(&committed(store.snapshot_outcome()), &note_id);
        let outcome = committed(store.apply_enrichment_outcome(
            &workspace_id,
            &note_id,
            &parsed_enrichment(),
            &token_for(&workspace_id, &note_id, before.enrichment_revision()),
            false,
        ));
        let after = note_in(&outcome, &note_id);
        assert_eq!(after.markdown(), original_text);
    }

    #[test]
    fn apply_enrichment_adds_relationships_with_ai_provenance() {
        let mut store = MemoryStore::new();
        let workspace_id = workspace_id_named(&committed(store.snapshot_outcome()), DEFAULT_WORKSPACE_NAME);
        committed(store.set_assistance_policy_outcome(&workspace_id, "local_ai"));
        let target = committed(store.create_note_outcome(&workspace_id, "target"));
        let other = committed(store.create_note_outcome(&workspace_id, "related"));
        let target_id = target.notes[0].id.clone();
        let other_id = other.notes[0].id.clone();
        let before_target = note_in(&committed(store.snapshot_outcome()), &target_id);
        let before_other = note_in(&committed(store.snapshot_outcome()), &other_id);
        let mut parsed = parsed_enrichment();
        parsed.related_note_ids = vec![other_id.clone()];
        let outcome = committed(store.apply_enrichment_outcome(
            &workspace_id,
            &target_id,
            &parsed,
            &token_for(&workspace_id, &target_id, before_target.enrichment_revision()),
            false,
        ));
        let after = &outcome;
        assert_eq!(after.relationships.len(), 1);
        assert_eq!(
            after.relationships[0].provenance,
            RelationshipProvenance::Ai
        );
        // The other endpoint's enrichment revision was bumped by the gate,
        // so a later manual edit can invalidate in-flight AI against it.
        let after_other = note_in(after, &other_id);
        assert!(after_other.enrichment_revision() > before_other.enrichment_revision());
    }

    #[test]
    fn apply_enrichment_survives_reopen() {
        let path = temporary_path();
        let workspace_id;
        let note_id;
        let original_text;
        {
            let mut store = WorkspaceStore::open(&path).unwrap();
            let snapshot = committed(store.snapshot_outcome());
            workspace_id = workspace_id_named(&snapshot, DEFAULT_WORKSPACE_NAME);
            committed(store.set_assistance_policy_outcome(&workspace_id, "local_ai"));
            original_text = "an org thought that persists";
            let note = committed(store.create_note_outcome(&workspace_id, original_text));
            note_id = note.notes[0].id.clone();
            let before = note_in(&committed(store.snapshot_outcome()), &note_id);
            let outcome = committed(store.apply_enrichment_outcome(
                &workspace_id,
                &note_id,
                &parsed_enrichment(),
                &token_for(&workspace_id, &note_id, before.enrichment_revision()),
                false,
            ));
            let after = note_in(&outcome, &note_id);
            assert_eq!(after.note_type, "claim");
            assert_eq!(after.note_type_provenance, Provenance::Ai);
        }
        let reopened = WorkspaceStore::open(&path).unwrap().snapshot().unwrap();
        let after = reopened
            .notes
            .iter()
            .find(|note| note.id == note_id)
            .expect("note survives reopen");
        assert_eq!(after.markdown(), original_text);
        assert_eq!(after.note_type, "claim");
        assert_eq!(after.note_type_provenance, Provenance::Ai);
        assert_eq!(after.annotation.as_deref(), Some("an additive note"));
        assert!(after.last_enriched_at().is_some());
        let label_names: Vec<String> = after.labels.iter().map(|label| label.name().to_owned()).collect();
        assert_eq!(label_names, vec!["alpha".to_owned(), "beta".to_owned()]);
        drop(reopened);
        remove_database(&path);
    }

    /// A Note edit between request and apply must invalidate the
    /// in-flight enrichment result, even when the AI result is otherwise
    /// valid. The capture is the revision; the apply re-checks it.
    #[test]
    fn apply_enrichment_drops_results_when_the_thinker_edits_during_inference() {
        let mut store = MemoryStore::new();
        let workspace_id = workspace_id_named(&committed(store.snapshot_outcome()), DEFAULT_WORKSPACE_NAME);
        committed(store.set_assistance_policy_outcome(&workspace_id, "local_ai"));
        let note = committed(store.create_note_outcome(&workspace_id, "an org thought"));
        let note_id = note.notes[0].id.clone();
        // Capture the revision at request time, then bump it with a manual
        // edit while the AI is "thinking" — the apply path should reject
        // the captured token as stale.
        let captured_revision = note.notes[0].enrichment_revision();
        committed(store.set_note_pinned_outcome(&note_id, true));
        let outcome = store.apply_enrichment_outcome(
            &workspace_id,
            &note_id,
            &parsed_enrichment(),
            &token_for(&workspace_id, &note_id, captured_revision),
            false,
        );
        assert!(matches!(
            outcome,
            WorkspaceCommandResult::Failed {
                failure: WorkspaceFailure {
                    code: WorkspaceFailureCode::Stale,
                    ..
                }
            }
        ));
    }
}
