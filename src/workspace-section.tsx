import type { FormEvent } from "react"
import type { ThinkingWorkspace } from "./workspace-client"

/** The Thinking Workspaces the thinker can switch between, and the one form that creates another. */
export function WorkspaceSection({
  workspaces,
  activeWorkspaceId,
  name,
  onSelect,
  onNameChange,
  onCreate,
}: {
  workspaces: ThinkingWorkspace[]
  activeWorkspaceId: string | undefined
  name: string
  onSelect: (workspaceId: string) => void
  onNameChange: (name: string) => void
  onCreate: (event: FormEvent) => void
}) {
  return (
    <section aria-label="Thinking Workspaces">
      <div className="workspace-list">
        {workspaces.map((workspace) => (
          <button
            className={workspace.id === activeWorkspaceId ? "active" : ""}
            key={workspace.id}
            onClick={() => onSelect(workspace.id)}
          >
            {workspace.name}
          </button>
        ))}
      </div>
      <form onSubmit={onCreate}>
        <input aria-label="New Thinking Workspace name" value={name} onChange={(event) => onNameChange(event.target.value)} placeholder="New Thinking Workspace" />
        <button type="submit">Create Workspace</button>
      </form>
    </section>
  )
}
