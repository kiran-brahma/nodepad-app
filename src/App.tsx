import { FormEvent, useCallback, useEffect, useMemo, useState } from "react"
import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"
import {
  NOTE_TYPES,
  thinkingWorkspace,
  type Note,
  type NoteType,
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
import {
  annotationLength,
  isAnnotationTooLong,
  isUndoShortcut,
  MAX_ANNOTATION_SCALARS,
  noteDeleteConfirmationPrompt,
  noteTypeLabel,
  requestNoteDelete,
  resolveNoteDeleteConfirmation,
  type PendingNoteDelete,
} from "./note-controls"

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
  const [pendingNoteDelete, setPendingNoteDelete] = useState<PendingNoteDelete>(null)
  const [noteDraft, setNoteDraft] = useState<{ id: string; markdown: string } | null>(null)
  const [annotationDraft, setAnnotationDraft] = useState<{ id: string; text: string } | null>(null)
  const [failure, setFailure] = useState<WorkspaceFailure | null>(null)

  const activeWorkspace = useMemo(
    () => snapshot?.workspaces.find(({ id }) => id === snapshot.activeWorkspaceId),
    [snapshot],
  )
  const notes = snapshot?.notes.filter((note) => note.workspaceId === activeWorkspace?.id) ?? []

  const submit = useCallback(async (pending: Promise<WorkspaceOutcome>) => {
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
  }, [])

  useEffect(() => {
    void submit(thinkingWorkspace.getSnapshot())
  }, [submit])

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

  function startNoteEdit(note: Note) {
    setNoteDraft({ id: note.id, markdown: note.markdown })
  }

  function saveNoteText(event: FormEvent) {
    event.preventDefault()
    if (!noteDraft) return
    void submit(thinkingWorkspace.editNoteText(noteDraft.id, noteDraft.markdown)).then(
      (committed) => {
        if (committed) setNoteDraft(null)
      },
    )
  }

  function startAnnotation(note: Note) {
    setAnnotationDraft({ id: note.id, text: note.annotation ?? "" })
  }

  function saveAnnotation(event: FormEvent) {
    event.preventDefault()
    if (!annotationDraft || isAnnotationTooLong(annotationDraft.text)) return
    void submit(thinkingWorkspace.setNoteAnnotation(annotationDraft.id, annotationDraft.text)).then(
      (committed) => {
        if (committed) setAnnotationDraft(null)
      },
    )
  }

  function answerNoteDeleteConfirmation(answer: "confirm" | "cancel") {
    const resolution = resolveNoteDeleteConfirmation(pendingNoteDelete, answer)
    setPendingNoteDelete(null)
    if (resolution.intent === "none") return
    setNoteDraft(null)
    setAnnotationDraft(null)
    void submit(thinkingWorkspace.deleteNote(resolution.noteId))
  }

  const undoLastChange = useCallback(() => {
    if (!snapshot?.activeWorkspaceId) return
    void submit(thinkingWorkspace.undoLastChange(snapshot.activeWorkspaceId))
  }, [snapshot?.activeWorkspaceId, submit])

  // Undo is a keyboard habit, so it works anywhere except inside text the
  // thinker is still writing, where the field's own undo belongs.
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      const editing = document.activeElement
      const editingText =
        editing instanceof HTMLTextAreaElement ||
        (editing instanceof HTMLInputElement && editing.type === "text")
      const shortcut = {
        key: event.key,
        metaKey: event.metaKey,
        ctrlKey: event.ctrlKey,
        shiftKey: event.shiftKey,
        editingText,
      }
      if (!isUndoShortcut(shortcut)) return
      event.preventDefault()
      undoLastChange()
    }
    window.addEventListener("keydown", onKeyDown)
    return () => window.removeEventListener("keydown", onKeyDown)
  }, [undoLastChange])

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
        <div className="row">
          <h2>Committed Notes</h2>
          <button
            onClick={undoLastChange}
            disabled={!snapshot || snapshot.undoableCommands === 0}
            title="Undo the last change in this Thinking Workspace (⌘Z)"
          >
            Undo
          </button>
        </div>
        {notes.length === 0 ? (
          <p>No Notes yet.</p>
        ) : (
          <ul className="notes">
            {notes.map((note) => (
              <li key={note.id} className={note.pinned ? "note pinned" : "note"}>
                <div className="row">
                  <span className="badge">{noteTypeLabel(note.noteType)}</span>
                  {note.pinned && <span className="badge">Pinned</span>}
                </div>

                {noteDraft?.id === note.id ? (
                  <form onSubmit={saveNoteText}>
                    <label htmlFor={`note-text-${note.id}`}>Note text</label>
                    <textarea
                      autoFocus
                      id={`note-text-${note.id}`}
                      rows={5}
                      value={noteDraft.markdown}
                      onChange={(event) => setNoteDraft({ ...noteDraft, markdown: event.target.value })}
                    />
                    <div className="row">
                      <button type="submit">Save Note text</button>
                      <button type="button" onClick={() => setNoteDraft(null)}>
                        Cancel
                      </button>
                    </div>
                  </form>
                ) : (
                  // Markdown renders without raw HTML, so nothing in a Note executes.
                  <div className="markdown">
                    <ReactMarkdown remarkPlugins={[remarkGfm]}>{note.markdown}</ReactMarkdown>
                  </div>
                )}

                {annotationDraft?.id === note.id ? (
                  <form onSubmit={saveAnnotation}>
                    <label htmlFor={`annotation-${note.id}`}>Annotation</label>
                    <textarea
                      autoFocus
                      id={`annotation-${note.id}`}
                      rows={3}
                      value={annotationDraft.text}
                      placeholder="Plain-text commentary; leave empty to clear it"
                      onChange={(event) =>
                        setAnnotationDraft({ ...annotationDraft, text: event.target.value })
                      }
                    />
                    <p className={isAnnotationTooLong(annotationDraft.text) ? "over-limit" : ""}>
                      {annotationLength(annotationDraft.text)} / {MAX_ANNOTATION_SCALARS} characters
                    </p>
                    <div className="row">
                      <button type="submit" disabled={isAnnotationTooLong(annotationDraft.text)}>
                        Save Annotation
                      </button>
                      <button type="button" onClick={() => setAnnotationDraft(null)}>
                        Cancel
                      </button>
                    </div>
                  </form>
                ) : (
                  note.annotation && <p className="annotation">{note.annotation}</p>
                )}

                <div className="row">
                  <label htmlFor={`note-type-${note.id}`}>Note Type</label>
                  <select
                    id={`note-type-${note.id}`}
                    value={note.noteType}
                    onChange={(event) =>
                      void submit(
                        thinkingWorkspace.setNoteType(note.id, event.target.value as NoteType),
                      )
                    }
                  >
                    {NOTE_TYPES.map((noteType) => (
                      <option key={noteType} value={noteType}>
                        {noteTypeLabel(noteType)}
                      </option>
                    ))}
                  </select>
                  <button onClick={() => startNoteEdit(note)} disabled={noteDraft?.id === note.id}>
                    Edit Note
                  </button>
                  <button
                    onClick={() => startAnnotation(note)}
                    disabled={annotationDraft?.id === note.id}
                  >
                    {note.annotation ? "Edit Annotation" : "Add Annotation"}
                  </button>
                  <button
                    aria-pressed={note.pinned}
                    onClick={() => void submit(thinkingWorkspace.setNotePinned(note.id, !note.pinned))}
                  >
                    {note.pinned ? "Unpin" : "Pin"}
                  </button>
                  <button onClick={() => setPendingNoteDelete(requestNoteDelete(note))}>
                    Delete Note
                  </button>
                </div>

                {pendingNoteDelete?.noteId === note.id && (
                  <div className="confirm" role="alertdialog" aria-label="Confirm delete Note">
                    <p>{noteDeleteConfirmationPrompt(pendingNoteDelete)}</p>
                    <div className="row">
                      <button onClick={() => answerNoteDeleteConfirmation("confirm")}>
                        Delete Note
                      </button>
                      <button onClick={() => answerNoteDeleteConfirmation("cancel")}>Keep it</button>
                    </div>
                  </div>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>
    </main>
  )
}
