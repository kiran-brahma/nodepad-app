import { invoke } from "@tauri-apps/api/core"

/** The fixed structural classifications a Note may carry. */
export const NOTE_TYPES = [
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
] as const

export type NoteType = (typeof NOTE_TYPES)[number]

/** Whether a value is still the default or was chosen by the thinker. */
export type Provenance = "default" | "manual"

export interface Note {
  id: string
  workspaceId: string
  markdown: string
  noteType: NoteType
  noteTypeProvenance: Provenance
  annotation: string | null
  annotationProvenance: Provenance
  createdAt: string
  updatedAt: string
  pinned: boolean
  labels: Label[]
}

export interface Label {
  id: string
  workspaceId: string
  name: string
}

export interface SearchResult {
  noteId: string
  snippet: string
  noteType: NoteType
  labels: Label[]
  rank: number
}

/** Who created a Relationship. Only `manual` is written today. */
export type RelationshipProvenance = "manual" | "ai"

/**
 * A symmetric, untyped association between two distinct Notes in one Thinking
 * Workspace. `noteIdA` sorts before `noteIdB` so a pair has one row; that
 * ordering is storage, never direction.
 */
export interface Relationship {
  id: string
  workspaceId: string
  noteIdA: string
  noteIdB: string
  provenance: RelationshipProvenance
  createdAt: string
}

/** The per-Workspace choice that governs AI assistance. */
export type AssistancePolicy = "manual" | "local_ai" | "cloud_ai"

export interface ThinkingWorkspace {
  id: string
  name: string
  assistancePolicy: AssistancePolicy
  /** The opaque model identifier last chosen for this Workspace, if any. */
  selectedModel: string | null
  createdAt: string
  updatedAt: string
}

export interface WorkspaceSnapshot {
  workspaces: ThinkingWorkspace[]
  notes: Note[]
  /** Every committed Relationship; each endpoint always names a Note above. */
  relationships: Relationship[]
  /** Always names a Workspace in `workspaces`; it is a preference, not content. */
  activeWorkspaceId: string
  /** Reversible changes left in this session for the active Workspace. */
  undoableCommands: number
}

export type WorkspaceFailure =
  | { code: "validation"; message: string }
  | { code: "not_found"; message: string }
  | { code: "nothing_to_undo"; message: string }
  | { code: "storage"; message: string }

export type StorageOpenFailure = {
  category: "unreadable" | "migration" | "initialization"
  message: string
}

export type WorkspaceOutcome =
  | { status: "committed"; snapshot: WorkspaceSnapshot }
  | { status: "failed"; failure: WorkspaceFailure }
  | { status: "unavailable"; failure: StorageOpenFailure }

export type SearchOutcome =
  | { status: "committed"; results: SearchResult[] }
  | { status: "failed"; failure: WorkspaceFailure }

export type DiscoveryFailureCode = "unavailable" | "timeout" | "malformed_response" | "empty_list"

export type DiscoveryOutcome =
  | { status: "committed"; models: string[] }
  | { status: "failed"; failure: { code: DiscoveryFailureCode; message: string } }

/** The UI's only durable-state interface; it never accesses SQLite directly. */
export const thinkingWorkspace = {
  getSnapshot: () => invoke<WorkspaceOutcome>("get_workspace_snapshot"),
  createWorkspace: (name: string) =>
    invoke<WorkspaceOutcome>("create_workspace", { name }),
  selectWorkspace: (workspaceId: string) =>
    invoke<WorkspaceOutcome>("select_workspace", { workspaceId }),
  renameWorkspace: (workspaceId: string, name: string) =>
    invoke<WorkspaceOutcome>("rename_workspace", { workspaceId, name }),
  deleteWorkspace: (workspaceId: string) =>
    invoke<WorkspaceOutcome>("delete_workspace", { workspaceId }),
  createNote: (workspaceId: string, markdown: string) =>
    invoke<WorkspaceOutcome>("create_note", { workspaceId, markdown }),
  editNoteText: (noteId: string, markdown: string) =>
    invoke<WorkspaceOutcome>("edit_note_text", { noteId, markdown }),
  setNoteType: (noteId: string, noteType: NoteType) =>
    invoke<WorkspaceOutcome>("set_note_type", { noteId, noteType }),
  setNoteAnnotation: (noteId: string, annotation: string) =>
    invoke<WorkspaceOutcome>("set_note_annotation", { noteId, annotation }),
  setNotePinned: (noteId: string, pinned: boolean) =>
    invoke<WorkspaceOutcome>("set_note_pinned", { noteId, pinned }),
  deleteNote: (noteId: string) => invoke<WorkspaceOutcome>("delete_note", { noteId }),
  /**
   * Moves a Note into another Thinking Workspace: same identity and authored
   * fields, Labels remapped by display meaning, and no Relationship, because a
   * Relationship never crosses a Workspace.
   */
  moveNote: (noteId: string, targetWorkspaceId: string) =>
    invoke<WorkspaceOutcome>("move_note", { noteId, targetWorkspaceId }),
  /** Copies a Note into another Thinking Workspace under a fresh identity. */
  copyNote: (noteId: string, targetWorkspaceId: string) =>
    invoke<WorkspaceOutcome>("copy_note", { noteId, targetWorkspaceId }),
  attachLabel: (noteId: string, name: string) =>
    invoke<WorkspaceOutcome>("attach_label", { noteId, name }),
  detachLabel: (noteId: string, labelId: string) =>
    invoke<WorkspaceOutcome>("detach_label", { noteId, labelId }),
  renameLabel: (labelId: string, name: string) =>
    invoke<WorkspaceOutcome>("rename_label", { labelId, name }),
  removeLabel: (labelId: string) => invoke<WorkspaceOutcome>("remove_label", { labelId }),
  /** Creating a Relationship that already exists commits no second one. */
  relateNotes: (noteId: string, otherNoteId: string) =>
    invoke<WorkspaceOutcome>("relate_notes", { noteId, otherNoteId }),
  unrelateNotes: (noteId: string, otherNoteId: string) =>
    invoke<WorkspaceOutcome>("unrelate_notes", { noteId, otherNoteId }),
  searchNotes: (workspaceId: string, query: string) =>
    invoke<SearchOutcome>("search_notes", { workspaceId, query }),
  undoLastChange: (workspaceId: string) =>
    invoke<WorkspaceOutcome>("undo_last_change", { workspaceId }),
  /** Changes the Assistance Policy of the active Thinking Workspace. */
  setAssistancePolicy: (workspaceId: string, policy: AssistancePolicy) =>
    invoke<WorkspaceOutcome>("set_assistance_policy", { workspaceId, policy }),
  /**
   * Records the opaque model identifier chosen for the active Thinking
   * Workspace. Passing `null` clears the selection.
   */
  selectModel: (workspaceId: string, modelId: string | null) =>
    invoke<WorkspaceOutcome>("select_model", { workspaceId, modelId }),
  /** Discovers models from the fixed local Ollama host. */
  discoverLocalModels: () => invoke<DiscoveryOutcome>("discover_local_models"),
  retryStorageOpen: () => invoke<WorkspaceOutcome>("retry_storage_open"),
  quitApplication: () => invoke<void>("quit_application"),
}
