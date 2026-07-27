import { useState, type FormEvent } from "react"
import type { ThinkingWorkspace } from "./workspace-client"
import { useEscape, ESCAPE_PRIORITY } from "./escape-stack"

/**
 * The rail's Thinking Workspaces: a vertical list marking the active one,
 * in-place rename on a row, and a create control in the rail footer beside
 * the Settings entry.
 *
 * It owns no business rule. Selecting, creating, and renaming all run App's
 * handlers over the existing commands; the rename draft is App's, shared with
 * the settings sheet so the two surfaces cannot disagree about the name being
 * edited. The only state kept here is whether the create field is open, which
 * nothing outside the rail needs to know.
 */
export function WorkspaceSection({
  workspaces,
  activeWorkspaceId,
  name,
  renameDraft,
  onSelect,
  onNameChange,
  onCreate,
  onStartRename,
  onRenameDraftChange,
  onRename,
  onCancelRename,
  onOpenSettings,
}: {
  workspaces: ThinkingWorkspace[]
  activeWorkspaceId: string | undefined
  /** The new-Workspace name draft, owned by App and cleared by it on commit. */
  name: string
  /** The Workspace being renamed, shared with the settings sheet. */
  renameDraft: { id: string; name: string } | null
  onSelect: (workspaceId: string) => void
  onNameChange: (name: string) => void
  /** Commits the create and reports whether it committed, so the inline
   *  field closes only on a commit and a refusal keeps the name. */
  onCreate: () => Promise<boolean>
  onStartRename: (workspace: ThinkingWorkspace) => void
  onRenameDraftChange: (name: string) => void
  onRename: (event: FormEvent) => void
  onCancelRename: () => void
  /** Opens the Workspace settings sheet for the active Workspace. */
  onOpenSettings: () => void
}) {
  const [creating, setCreating] = useState(false)

  function create(event: FormEvent) {
    event.preventDefault()
    void onCreate().then((committed) => {
      if (committed) setCreating(false)
    })
  }

  return (
    <section aria-label="Thinking Workspaces">
      <ul className="workspace-list">
        {workspaces.map((workspace) => (
          <li
            key={workspace.id}
            className={workspace.id === activeWorkspaceId ? "workspace-row active" : "workspace-row"}
          >
            {renameDraft?.id === workspace.id ? (
              <RailRenameForm
                draft={renameDraft}
                onDraftChange={onRenameDraftChange}
                onSubmit={onRename}
                onCancel={onCancelRename}
              />
            ) : (
              <>
                <button
                  className="workspace-row-name"
                  aria-current={workspace.id === activeWorkspaceId ? "true" : undefined}
                  onClick={() => onSelect(workspace.id)}
                  onDoubleClick={() => onStartRename(workspace)}
                >
                  {workspace.name}
                </button>
                <button
                  className="workspace-row-action"
                  aria-label={`Rename ${workspace.name}`}
                  onClick={() => onStartRename(workspace)}
                >
                  Rename
                </button>
              </>
            )}
          </li>
        ))}
      </ul>
      <div className="workspace-rail-footer">
        {creating ? (
          <form onSubmit={create}>
            <input
              autoFocus
              aria-label="New Thinking Workspace name"
              value={name}
              onChange={(event) => onNameChange(event.target.value)}
              placeholder="New Thinking Workspace"
            />
            <button type="submit">Create Workspace</button>
          </form>
        ) : (
          <button onClick={() => setCreating(true)}>New Workspace</button>
        )}
        <button
          onClick={onOpenSettings}
          disabled={!activeWorkspaceId}
          aria-label="Workspace settings"
        >
          Settings
        </button>
      </div>
    </section>
  )
}

/** In-place rename on a rail row. Escape cancels through the shared escape
 *  stack, at the same priority as the settings sheet's rename form, so a
 *  cancel clears the one draft wherever it was started. */
function RailRenameForm({
  draft,
  onDraftChange,
  onSubmit,
  onCancel,
}: {
  draft: { id: string; name: string }
  onDraftChange: (name: string) => void
  onSubmit: (event: FormEvent) => void
  onCancel: () => void
}) {
  useEscape(onCancel, ESCAPE_PRIORITY.dialog)
  return (
    <form onSubmit={onSubmit}>
      <input
        autoFocus
        aria-label="Rename Thinking Workspace"
        value={draft.name}
        onChange={(event) => onDraftChange(event.target.value)}
      />
      <button type="submit">Save Workspace name</button>
    </form>
  )
}
