import { useState, type FormEvent, type ReactNode } from "react"
import type { ThinkingWorkspace } from "./workspace-client"
import { useEscape, ESCAPE_PRIORITY } from "./escape-stack"
import { WorkspaceRenameForm, type WorkspaceRenameDraft } from "./workspace-rename-form"

/**
 * The rail's Thinking Workspaces: a vertical list marking the active one,
 * in-place rename on a row, and a create control in the rail footer beside
 * the Settings entry.
 *
 * It owns no business rule. Selecting, creating, and renaming all run App's
 * handlers over the existing commands, and the rail's rename draft is the
 * rail's alone, so a rename started in the settings sheet never opens a field
 * on a row behind it. The only state kept here is whether the create field is
 * open, which nothing outside the rail needs to know.
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
  /** The Workspace being renamed from a rail row, if any. */
  renameDraft: WorkspaceRenameDraft | null
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

  /** Escape abandons the create: the field closes and the name goes with it,
   *  so reopening it never resumes a name the thinker walked away from. */
  function cancelCreate() {
    setCreating(false)
    onNameChange("")
  }

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
              <WorkspaceRenameForm
                draft={renameDraft}
                fieldLabel="Rename Thinking Workspace"
                submitLabel="Save Workspace name"
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
          <CreateWorkspaceForm onSubmit={create} onCancel={cancelCreate}>
            <input
              autoFocus
              aria-label="New Thinking Workspace name"
              value={name}
              onChange={(event) => onNameChange(event.target.value)}
              placeholder="New Thinking Workspace"
            />
            <button type="submit">Create Workspace</button>
          </CreateWorkspaceForm>
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

/** The rail footer's create field. Escape dismisses it through the shared
 *  escape stack, at the same priority as an inline rename. */
function CreateWorkspaceForm({
  onSubmit,
  onCancel,
  children,
}: {
  onSubmit: (event: FormEvent) => void
  onCancel: () => void
  children: ReactNode
}) {
  useEscape(onCancel, ESCAPE_PRIORITY.dialog)
  return <form onSubmit={onSubmit}>{children}</form>
}
