import { invoke } from "@tauri-apps/api/core"

export type NoteType = "general"

export interface Note {
  id: string
  workspaceId: string
  markdown: string
  noteType: NoteType
  createdAt: string
  updatedAt: string
  pinned: boolean
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
}

export type WorkspaceFailure =
  | { code: "validation"; message: string }
  | { code: "not_found"; message: string }
  | { code: "storage"; message: string }

export type WorkspaceOutcome =
  | { status: "committed"; snapshot: WorkspaceSnapshot }
  | { status: "failed"; failure: WorkspaceFailure }

/** The UI's only durable-state interface; it never accesses SQLite directly. */
export const thinkingWorkspace = {
  getSnapshot: () => invoke<WorkspaceOutcome>("get_workspace_snapshot"),
  createWorkspace: (name: string) =>
    invoke<WorkspaceOutcome>("create_workspace", { name }),
  createNote: (workspaceId: string, markdown: string) =>
    invoke<WorkspaceOutcome>("create_note", { workspaceId, markdown }),
}
