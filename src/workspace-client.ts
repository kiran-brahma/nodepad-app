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

export interface ThinkingWorkspace {
  id: string
  name: string
  createdAt: string
  updatedAt: string
}

export interface WorkspaceSnapshot {
  workspaces: ThinkingWorkspace[]
  notes: Note[]
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
  attachLabel: (noteId: string, name: string) =>
    invoke<WorkspaceOutcome>("attach_label", { noteId, name }),
  detachLabel: (noteId: string, labelId: string) =>
    invoke<WorkspaceOutcome>("detach_label", { noteId, labelId }),
  renameLabel: (labelId: string, name: string) =>
    invoke<WorkspaceOutcome>("rename_label", { labelId, name }),
  removeLabel: (labelId: string) => invoke<WorkspaceOutcome>("remove_label", { labelId }),
  searchNotes: (workspaceId: string, query: string) =>
    invoke<SearchOutcome>("search_notes", { workspaceId, query }),
  undoLastChange: (workspaceId: string) =>
    invoke<WorkspaceOutcome>("undo_last_change", { workspaceId }),
  retryStorageOpen: () => invoke<WorkspaceOutcome>("retry_storage_open"),
  quitApplication: () => invoke<void>("quit_application"),
}
