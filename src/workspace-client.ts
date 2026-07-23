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

/** Whether a value is still the default, was chosen by the thinker, or
 *  was suggested by AI. AI-authored fields are visibly distinguishable
 *  in the UI and a later AI result may refresh them; a manual field
 *  always wins until the thinker explicitly chooses Re-enrich and
 *  Replace. */
export type Provenance = "default" | "manual" | "ai"

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
  /** Bumped on every commit that touches this Note. The Enrichment
   *  Workflow captures it into the request token and rejects any
   *  response that names a different revision, so an edit during
   *  inference invalidates the in-flight response. */
  enrichmentRevision: number
  /** The moment a successful AI organization was last applied, if any. */
  lastEnrichedAt: string | null
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

/**
 * A provisional insight connecting several Notes, waiting for the thinker to
 * accept or dismiss it. It is not a Note: it has no place in the Thinking
 * Graph, no Relationship is written for it, and it changes nothing it names.
 */
export interface PendingSynthesis {
  id: string
  workspaceId: string
  text: string
  /** The exact source Notes the model named, in the order it named them. */
  sourceNoteIds: string[]
  labels: string[]
  /** AI provenance: which model proposed this, under which policy. */
  model: string
  policy: AssistancePolicy
  createdAt: string
  /**
   * True when a source Note was edited, deleted, or moved to another Thinking
   * Workspace since the Synthesis was proposed. A stale Synthesis is still
   * shown and can still be dismissed, but it can no longer be accepted.
   */
  stale: boolean
}

/** The per-Workspace choice that governs AI assistance. */
export type AssistancePolicy = "manual" | "local_ai" | "cloud_ai"

export interface ThinkingWorkspace {
  id: string
  name: string
  assistancePolicy: AssistancePolicy
  /** The opaque model identifier last chosen for this Workspace, if any. */
  selectedModel: string | null
  /**
   * When the thinker first accepted the Cloud AI disclosure for this Workspace.
   * A string when consent was given (the moment is the receipt); null when
   * the Workspace is not yet consented, or when consent was revoked. The
   * bearer key itself is never in this record.
   */
  cloudConsentAt: string | null
  createdAt: string
  updatedAt: string
}

/**
 * Whether a Thinking Workspace's Assistance Policy permits an AI call at
 * all. One predicate, so Note Organization, Synthesis, and the panels can
 * never disagree about whether a Workspace is Manual.
 */
export function assistanceEnabled(workspace: ThinkingWorkspace | undefined): boolean {
  if (!workspace) return false
  return workspace.assistancePolicy !== "manual"
}

export interface WorkspaceSnapshot {
  workspaces: ThinkingWorkspace[]
  notes: Note[]
  /** Every committed Relationship; each endpoint always names a Note above. */
  relationships: Relationship[]
  /** Every undecided Synthesis, in proposal order. Provisional, never Notes. */
  pendingSyntheses: PendingSynthesis[]
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
  | { code: "stale"; message: string }

export type StorageOpenFailure = {
  category: "unreadable" | "migration" | "initialization"
  message: string
}

/** Why a Thinking Workspace's durable data was backed up. Automatic backups
 *  run at most once per local calendar day after data changed; pre-migration
 *  and pre-restore backups are explicit and kept separately. */
export type BackupKind = "automatic" | "pre_migration" | "pre_restore"

/** One valid local backup in the macOS application-data folder. The recovery
 *  screen lists these; restore re-validates checksum, integrity, and schema
 *  before replacing current data. */
export interface BackupRecord {
  id: string
  kind: BackupKind
  schemaVersion: number
  createdAt: string
  appVersion: string
  checksum: string
}

/** Why a restore did not replace current data. Each code is a typed recovery
 *  state; the current database is preserved on every failure. */
export type RestoreFailureCode =
  | "not_found"
  | "checksum_mismatch"
  | "corrupt"
  | "unsupported_schema"
  | "pre_restore_failed"
  | "replacement_failed"
  | "reopen_failed"
  | "unavailable"

/** The result of restoring one backup after explicit confirmation. A restored
 *  database carries the new committed snapshot. */
export type RestoreOutcome =
  | { status: "restored"; snapshot: WorkspaceSnapshot }
  | { status: "failed"; code: RestoreFailureCode; message: string }

export type WorkspaceOutcome =
  | { status: "committed"; snapshot: WorkspaceSnapshot }
  | { status: "failed"; failure: WorkspaceFailure }
  | { status: "unavailable"; failure: StorageOpenFailure }

export type SearchOutcome =
  | { status: "committed"; results: SearchResult[] }
  | { status: "failed"; failure: WorkspaceFailure }

export type MarkdownExportOutcome =
  | { status: "exported"; filename: string }
  | { status: "cancelled" }
  | { status: "failed"; message: string }

/** The result of exporting one Thinking Workspace as a versioned Nodepad
 *  archive. A cancel of the native save dialog is a successful no-op. */
export type ArchiveExportOutcome =
  | { status: "exported"; filename: string }
  | { status: "cancelled" }
  | { status: "failed"; message: string }

/** The result of importing a versioned Nodepad archive. A cancel of the open
 *  dialog is a successful no-op; a malformed archive fails closed. */
export type ArchiveImportOutcome =
  | { status: "imported"; snapshot: WorkspaceSnapshot }
  | { status: "cancelled" }
  | { status: "failed"; message: string }

export type DiscoveryFailureCode =
  | "unavailable"
  | "timeout"
  | "malformed_response"
  | "empty_list"
  /** Cloud only: there is no key in the macOS keychain right now. */
  | "unauthenticated"
  /** Cloud only: the cloud host rejected the saved key. */
  | "authentication_failed"
  /** Cloud only: the cloud host is throttling requests. */
  | "rate_limited"

export type DiscoveryState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "ready"; models: string[] }
  | { kind: "error"; failure: { code: DiscoveryFailureCode; message: string } }

export type DiscoveryOutcome =
  | { status: "committed"; models: string[] }
  | { status: "failed"; failure: { code: DiscoveryFailureCode; message: string } }

/** The result of writing or deleting the bearer key in the keychain. */
export type CloudSecretOutcome =
  | { status: "ok" }
  | { status: "failed"; failure: { code: "unavailable" | "refused"; message: string } }

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
  exportWorkspace: (workspaceId: string) =>
    invoke<MarkdownExportOutcome>("export_workspace", { workspaceId }),
  /** Exports one Thinking Workspace as a versioned Nodepad archive whose
   *  durable bytes carry no secrets or transient state. */
  exportWorkspaceArchive: (workspaceId: string) =>
    invoke<ArchiveExportOutcome>("export_workspace_archive", { workspaceId }),
  /** Imports one validated V0 archive as a fresh Thinking Workspace. The
   *  complete archive is validated before any durable row is touched, so a
   *  malformed archive fails closed and changes nothing. */
  importWorkspaceArchive: () =>
    invoke<ArchiveImportOutcome>("import_workspace_archive"),
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
  /**
   * Records or revokes the Workspace's affirmative consent to use Ollama
   * Cloud. `accept` true records the moment of first consent; false clears
   * it. The bearer key is never stored in the database.
   */
  setCloudConsent: (workspaceId: string, accept: boolean) =>
    invoke<WorkspaceOutcome>("set_cloud_consent", { workspaceId, accept }),
  /**
   * Saves the bearer key to the macOS keychain. The key is read on demand
   * for cloud discovery; this command does not echo it back.
   */
  setCloudApiKey: (apiKey: string) =>
    invoke<CloudSecretOutcome>("set_cloud_api_key", { apiKey }),
  /** Removes the bearer key from the macOS keychain. */
  deleteCloudApiKey: () => invoke<CloudSecretOutcome>("delete_cloud_api_key"),
  /** Whether a key is currently in the keychain. */
  cloudApiKeyPresent: () => invoke<boolean>("cloud_api_key_present"),
  /** Discovers models from the fixed Ollama Cloud host. */
  discoverCloudModels: (workspaceId: string) =>
    invoke<DiscoveryOutcome>("discover_cloud_models", { workspaceId }),
  retryStorageOpen: () => invoke<WorkspaceOutcome>("retry_storage_open"),
  /** Lists the valid local backups, newest first. Available even when storage
   *  would not open, so the recovery screen can offer a restore. */
  listBackups: () => invoke<BackupRecord[]>("list_backups"),
  /** Restores one backup after the UI confirms. Re-validates the backup, makes
   *  a pre-restore backup of the current database, swaps the files, and
   *  reopens durable state. An invalid backup never replaces data. */
  restoreBackup: (backupId: string) =>
    invoke<RestoreOutcome>("restore_backup", { backupId }),
  quitApplication: () => invoke<void>("quit_application"),
}
