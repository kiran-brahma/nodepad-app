import { FormEvent, useCallback, useMemo, useState } from "react"
import { thinkingWorkspace, type SearchResult, type ThinkingWorkspace } from "./workspace-client"
import { requestDelete, resolveDeleteConfirmation, type PendingDelete } from "./workspace-lifecycle"
import { NoteCard, type NoteCardContext } from "./note-card"
import { buildNoteIntents } from "./note-intents"
import { useNoteDrafts } from "./note-drafts"
import { useNoteFocus } from "./note-focus"
import { useWorkspaceSnapshot } from "./workspace-snapshot"
import { useUndoShortcut } from "./undo-shortcut"
import { WorkspaceSection } from "./workspace-section"
import { CaptureSection } from "./capture-section"
import { SearchSection } from "./search-section"
import { StorageRecovery } from "./storage-recovery"

export function App() {
  const { snapshot, openFailure, failure, submit, reportFailure, dismissFailure } =
    useWorkspaceSnapshot()
  const drafts = useNoteDrafts()
  const focus = useNoteFocus()
  const [workspaceName, setWorkspaceName] = useState("")
  const [noteMarkdown, setNoteMarkdown] = useState("")
  const [renameDraft, setRenameDraft] = useState<{ id: string; name: string } | null>(null)
  const [pendingDelete, setPendingDelete] = useState<PendingDelete>(null)
  const [renameLabelDraft, setRenameLabelDraft] = useState<{ id: string; name: string } | null>(null)
  const [searchQuery, setSearchQuery] = useState("")
  const [searchResults, setSearchResults] = useState<SearchResult[] | null>(null)

  const activeWorkspace = useMemo(
    () => snapshot?.workspaces.find(({ id }) => id === snapshot.activeWorkspaceId),
    [snapshot],
  )
  const notes = snapshot?.notes.filter((note) => note.workspaceId === activeWorkspace?.id) ?? []
  const workspaces = snapshot?.workspaces ?? []
  const cardContext: NoteCardContext = {
    notes,
    relationships: snapshot?.relationships ?? [],
    workspaces,
  }

  // One set of Note intents, built once and handed to every card, so no view
  // can grow its own copy of what changing a Note means.
  const noteIntents = buildNoteIntents({
    drafts,
    workspaces,
    submit,
    focusNote: focus.focusNote,
    startLabelRename: (label) => setRenameLabelDraft({ id: label.id, name: label.name }),
  })

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

  function createNote(event: FormEvent) {
    event.preventDefault()
    if (!activeWorkspace) return
    void submit(thinkingWorkspace.createNote(activeWorkspace.id, noteMarkdown)).then((committed) => {
      if (committed) setNoteMarkdown("")
    })
  }

  function saveRenamedLabel(event: FormEvent) {
    event.preventDefault()
    if (!renameLabelDraft) return
    void submit(thinkingWorkspace.renameLabel(renameLabelDraft.id, renameLabelDraft.name)).then((committed) => {
      if (committed) setRenameLabelDraft(null)
    })
  }

  function search(event: FormEvent) {
    event.preventDefault()
    if (!activeWorkspace || searchQuery.trim() === "") {
      setSearchResults(null)
      return
    }
    void thinkingWorkspace.searchNotes(activeWorkspace.id, searchQuery).then((outcome) => {
      if (outcome.status === "failed") { reportFailure(outcome.failure); return }
      setSearchResults(outcome.results)
    })
  }

  const undoLastChange = useCallback(() => {
    if (!snapshot?.activeWorkspaceId) return
    void submit(thinkingWorkspace.undoLastChange(snapshot.activeWorkspaceId))
  }, [snapshot?.activeWorkspaceId, submit])

  useUndoShortcut(undoLastChange)

  if (openFailure) {
    return (
      <StorageRecovery
        failure={openFailure}
        onRetry={() => void submit(thinkingWorkspace.retryStorageOpen())}
        onQuit={() => void thinkingWorkspace.quitApplication()}
      />
    )
  }

  return (
    <main>
      <header>
        <p className="eyebrow">Nodepad</p>
        <h1>Thinking Workspace</h1>
        <p>Capture one atomic thought at a time. Every change is committed locally before it appears here.</p>
      </header>

      {failure && <aside role="alert">{failure.message} <button onClick={dismissFailure}>Dismiss</button></aside>}

      <WorkspaceSection
        workspaces={workspaces}
        activeWorkspaceId={activeWorkspace?.id}
        name={workspaceName}
        onSelect={(workspaceId) => void submit(thinkingWorkspace.selectWorkspace(workspaceId))}
        onNameChange={setWorkspaceName}
        onCreate={createWorkspace}
      />

      <CaptureSection
        activeWorkspace={activeWorkspace}
        renameDraft={renameDraft}
        pendingDelete={pendingDelete}
        noteMarkdown={noteMarkdown}
        onStartRename={(workspace: ThinkingWorkspace) =>
          setRenameDraft({ id: workspace.id, name: workspace.name })
        }
        onRenameDraftChange={(name) => setRenameDraft((draft) => (draft ? { ...draft, name } : draft))}
        onRename={renameWorkspace}
        onCancelRename={() => setRenameDraft(null)}
        onRequestDelete={(workspace) => setPendingDelete(requestDelete(workspace))}
        onAnswerDelete={answerDeleteConfirmation}
        onNoteMarkdownChange={setNoteMarkdown}
        onCreateNote={createNote}
      />

      <SearchSection
        query={searchQuery}
        results={searchResults}
        canSearch={Boolean(activeWorkspace)}
        onQueryChange={setSearchQuery}
        onSearch={search}
        onClear={() => { setSearchQuery(""); setSearchResults(null) }}
      />

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
              <NoteCard
                key={note.id}
                note={note}
                context={cardContext}
                drafts={drafts}
                intents={noteIntents}
                focused={focus.focusedNoteId === note.id}
                registerElement={(element) => focus.registerNoteElement(note.id, element)}
              />
            ))}
          </ul>
        )}
      </section>
      {renameLabelDraft && <section role="dialog" aria-label="Rename Label"><form onSubmit={saveRenamedLabel}><label htmlFor="rename-label">Label name</label><input autoFocus id="rename-label" value={renameLabelDraft.name} onChange={(event) => setRenameLabelDraft({ ...renameLabelDraft, name: event.target.value })} /><button type="submit">Save Label name</button><button type="button" onClick={() => setRenameLabelDraft(null)}>Cancel</button></form></section>}
    </main>
  )
}
