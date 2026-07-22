import { FormEvent, useEffect, useMemo, useState } from "react"
import {
  thinkingWorkspace,
  type StorageOpenFailure,
  type ThinkingWorkspace,
  type WorkspaceOutcome,
  type WorkspaceFailure,
  type WorkspaceSnapshot,
} from "./workspace-client"
import {
  deleteConfirmationPrompt,
  requestDelete,
  resolveDeleteConfirmation,
  type PendingDelete,
} from "./workspace-lifecycle"

const RECOVERY_HEADLINE: Record<StorageOpenFailure["category"], string> = {
  unreadable: "Nodepad could not read its local database.",
  migration: "Nodepad could not prepare its local database.",
  initialization: "Nodepad could not start its local storage.",
}

export function App() {
  const [snapshot, setSnapshot] = useState<WorkspaceSnapshot | null>(null)
  const [openFailure, setOpenFailure] = useState<StorageOpenFailure | null>(null)
  const [workspaceName, setWorkspaceName] = useState("")
  const [noteMarkdown, setNoteMarkdown] = useState("")
  const [renameDraft, setRenameDraft] = useState<{ id: string; name: string } | null>(null)
  const [pendingDelete, setPendingDelete] = useState<PendingDelete>(null)
  const [failure, setFailure] = useState<WorkspaceFailure | null>(null)

  useEffect(() => {
    void submit(thinkingWorkspace.getSnapshot())
  }, [])

  const activeWorkspace = useMemo(
    () => snapshot?.workspaces.find(({ id }) => id === snapshot.activeWorkspaceId),
    [snapshot],
  )
  const notes = snapshot?.notes.filter((note) => note.workspaceId === activeWorkspace?.id) ?? []

  async function submit(pending: Promise<WorkspaceOutcome>) {
    const outcome = await pending
    if (outcome.status === "unavailable") {
      setOpenFailure(outcome.failure)
      return false
    }
    if (outcome.status === "failed") {
      setFailure(outcome.failure)
      return false
    }
    setSnapshot(outcome.snapshot)
    setOpenFailure(null)
    setFailure(null)
    return true
  }

  function createWorkspace(event: FormEvent) {
    event.preventDefault()
    void submit(thinkingWorkspace.createWorkspace(workspaceName)).then((committed) => {
      if (committed) setWorkspaceName("")
    })
  }

  function renameWorkspace(event: FormEvent) {
    event.preventDefault()
    if (!renameDraft) return
    void submit(thinkingWorkspace.renameWorkspace(renameDraft.id, renameDraft.name)).then(
      (committed) => {
        if (committed) setRenameDraft(null)
      },
    )
  }

  function answerDeleteConfirmation(answer: "confirm" | "cancel") {
    const resolution = resolveDeleteConfirmation(pendingDelete, answer)
    setPendingDelete(null)
    if (resolution.intent === "none") return
    void submit(thinkingWorkspace.deleteWorkspace(resolution.workspaceId))
  }

  function startRename(workspace: ThinkingWorkspace) {
    setRenameDraft({ id: workspace.id, name: workspace.name })
  }

  function createNote(event: FormEvent) {
    event.preventDefault()
    if (!activeWorkspace) return
    void submit(thinkingWorkspace.createNote(activeWorkspace.id, noteMarkdown)).then((committed) => {
      if (committed) setNoteMarkdown("")
    })
  }

  if (openFailure) {
    return (
      <main>
        <header>
          <p className="eyebrow">Nodepad</p>
          <h1>Your thinking is still on disk</h1>
        </header>
        <section role="alert" className="recovery">
          <h2>{RECOVERY_HEADLINE[openFailure.category]}</h2>
          <p>{openFailure.message}</p>
          <p>
            Nothing has been reset or overwritten. Close anything else using this database, then try
            again.
          </p>
          <div className="row">
            <button onClick={() => void submit(thinkingWorkspace.retryStorageOpen())}>
              Try again
            </button>
            <button onClick={() => void thinkingWorkspace.quitApplication()}>Quit Nodepad</button>
          </div>
        </section>
      </main>
    )
  }

  return (
    <main>
      <header>
        <p className="eyebrow">Nodepad</p>
        <h1>Thinking Workspace</h1>
        <p>Capture one atomic thought at a time. Every change is committed locally before it appears here.</p>
      </header>

      {failure && <aside role="alert">{failure.message} <button onClick={() => setFailure(null)}>Dismiss</button></aside>}

      <section aria-label="Thinking Workspaces">
        <div className="workspace-list">
          {snapshot?.workspaces.map((workspace) => (
            <button
              className={workspace.id === activeWorkspace?.id ? "active" : ""}
              key={workspace.id}
              onClick={() => void submit(thinkingWorkspace.selectWorkspace(workspace.id))}
            >
              {workspace.name}
            </button>
          ))}
        </div>
        <form onSubmit={createWorkspace}>
          <input aria-label="New Thinking Workspace name" value={workspaceName} onChange={(event) => setWorkspaceName(event.target.value)} placeholder="New Thinking Workspace" />
          <button type="submit">Create Workspace</button>
        </form>
      </section>

      <section className="capture">
        <div className="row">
          <h2>{activeWorkspace?.name ?? "Loading…"}</h2>
          {activeWorkspace && !renameDraft && (
            <div className="row">
              <button onClick={() => startRename(activeWorkspace)}>Rename</button>
              <button onClick={() => setPendingDelete(requestDelete(activeWorkspace))}>Delete</button>
            </div>
          )}
        </div>

        {renameDraft && (
          <form onSubmit={renameWorkspace}>
            <label htmlFor="workspace-name">Thinking Workspace name</label>
            <input
              autoFocus
              id="workspace-name"
              value={renameDraft.name}
              onChange={(event) => setRenameDraft({ ...renameDraft, name: event.target.value })}
            />
            <div className="row">
              <button type="submit">Save name</button>
              <button type="button" onClick={() => setRenameDraft(null)}>Cancel</button>
            </div>
          </form>
        )}

        {pendingDelete && (
          <div className="confirm" role="alertdialog" aria-label="Confirm delete">
            <p>{deleteConfirmationPrompt(pendingDelete)}</p>
            <div className="row">
              <button onClick={() => answerDeleteConfirmation("confirm")}>Delete Workspace</button>
              <button onClick={() => answerDeleteConfirmation("cancel")}>Keep it</button>
            </div>
          </div>
        )}

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
