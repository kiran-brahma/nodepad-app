import { FormEvent, useEffect, useMemo, useState } from "react"
import {
  thinkingWorkspace,
  type WorkspaceOutcome,
  type WorkspaceFailure,
  type WorkspaceSnapshot,
} from "./workspace-client"

export function App() {
  const [snapshot, setSnapshot] = useState<WorkspaceSnapshot | null>(null)
  const [activeWorkspaceId, setActiveWorkspaceId] = useState("")
  const [workspaceName, setWorkspaceName] = useState("")
  const [noteMarkdown, setNoteMarkdown] = useState("")
  const [failure, setFailure] = useState<WorkspaceFailure | null>(null)

  useEffect(() => {
    void submit(thinkingWorkspace.getSnapshot())
  }, [])

  const activeWorkspace = useMemo(
    () => snapshot?.workspaces.find(({ id }) => id === activeWorkspaceId) ?? snapshot?.workspaces[0],
    [activeWorkspaceId, snapshot],
  )
  const notes = snapshot?.notes.filter((note) => note.workspaceId === activeWorkspace?.id) ?? []

  async function submit(pending: Promise<WorkspaceOutcome>) {
    const outcome = await pending
    if (outcome.status === "failed") {
      setFailure(outcome.failure)
      return false
    }
    setSnapshot(outcome.snapshot)
    setActiveWorkspaceId((current) => current || outcome.snapshot.workspaces[0]?.id || "")
    setFailure(null)
    return true
  }

  function createWorkspace(event: FormEvent) {
    event.preventDefault()
    void submit(thinkingWorkspace.createWorkspace(workspaceName)).then((committed) => {
      if (committed) setWorkspaceName("")
    })
  }

  function createNote(event: FormEvent) {
    event.preventDefault()
    if (!activeWorkspace) return
    void submit(thinkingWorkspace.createNote(activeWorkspace.id, noteMarkdown)).then((committed) => {
      if (committed) setNoteMarkdown("")
    })
  }

  return (
    <main>
      <header>
        <p className="eyebrow">Nodepad</p>
        <h1>Thinking Workspace</h1>
        <p>Capture one atomic thought at a time. Every change is committed locally before it appears here.</p>
      </header>

      {failure && <aside role="alert">{failure.message} <button onClick={() => setFailure(null)}>Dismiss</button></aside>}

      <section className="workspace-list" aria-label="Thinking Workspaces">
        {snapshot?.workspaces.map((workspace) => (
          <button className={workspace.id === activeWorkspace?.id ? "active" : ""} key={workspace.id} onClick={() => setActiveWorkspaceId(workspace.id)}>
            {workspace.name}
          </button>
        ))}
        <form onSubmit={createWorkspace}>
          <input aria-label="New Thinking Workspace name" value={workspaceName} onChange={(event) => setWorkspaceName(event.target.value)} placeholder="New Thinking Workspace" />
          <button type="submit">Create Workspace</button>
        </form>
      </section>

      <section className="capture">
        <h2>{activeWorkspace?.name ?? "Loading…"}</h2>
        <form onSubmit={createNote}>
          <label htmlFor="note">New Note</label>
          <textarea id="note" value={noteMarkdown} onChange={(event) => setNoteMarkdown(event.target.value)} placeholder="Write an atomic Markdown Note…" rows={5} />
          <button type="submit" disabled={!activeWorkspace}>Commit Note</button>
        </form>
      </section>

      <section aria-label="Committed Notes">
        <h2>Committed Notes</h2>
        {notes.length === 0 ? <p>No Notes yet.</p> : <ul>{notes.map((note) => <li key={note.id}>{note.markdown}</li>)}</ul>}
      </section>
    </main>
  )
}
