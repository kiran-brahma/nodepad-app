import { FormEvent, useCallback, useMemo, useState } from "react"
import {
  thinkingWorkspace,
  type AssistancePolicy,
  type Note,
  type SearchResult,
  type ThinkingWorkspace,
} from "./workspace-client"
import { requestDelete, resolveDeleteConfirmation, type PendingDelete } from "./workspace-lifecycle"
import { matchingNoteIds, visibleNotes, workspaceNotes, type NoteView } from "./note-views"
import { NoteCard, type NoteCardContext } from "./note-card"
import { buildNoteIntents } from "./note-intents"
import { useNoteDrafts } from "./note-drafts"
import { useNoteFocus } from "./note-focus"
import { thinkingGraph } from "./thinking-graph"
import { useWorkspaceSnapshot } from "./workspace-snapshot"
import { useUndoShortcut } from "./undo-shortcut"
import { WorkspaceSection } from "./workspace-section"
import { CaptureSection } from "./capture-section"
import { SearchSection } from "./search-section"
import { CommittedNotesSection } from "./committed-notes-section"
import { StorageRecovery } from "./storage-recovery"
import { AssistanceSection } from "./assistance-section"
import { useLocalDiscovery } from "./use-local-discovery"

export function App() {
  const { snapshot, openFailure, failure, submit, reportFailure, dismissFailure } =
    useWorkspaceSnapshot()
  const drafts = useNoteDrafts()
  const [workspaceName, setWorkspaceName] = useState("")
  const [noteMarkdown, setNoteMarkdown] = useState("")
  const [renameDraft, setRenameDraft] = useState<{ id: string; name: string } | null>(null)
  const [pendingDelete, setPendingDelete] = useState<PendingDelete>(null)
  const [renameLabelDraft, setRenameLabelDraft] = useState<{ id: string; name: string } | null>(null)
  const [searchQuery, setSearchQuery] = useState("")
  const [searchResults, setSearchResults] = useState<SearchResult[] | null>(null)
  // How the same committed Notes are arranged. Not committed, so a restart
  // reconstructs both views from SQLite alone.
  const [view, setView] = useState<NoteView>("tiling")

  const activeWorkspace = useMemo(
    () => snapshot?.workspaces.find(({ id }) => id === snapshot.activeWorkspaceId),
    [snapshot],
  )
  const notes = useMemo(
    () => workspaceNotes(snapshot?.notes ?? [], activeWorkspace?.id),
    [snapshot, activeWorkspace?.id],
  )
  const workspaces = snapshot?.workspaces ?? []
  // The one result set the arranged views read, so they can never disagree
  // about which Notes are on screen or in what order.
  const visible = useMemo(
    () => visibleNotes(snapshot?.notes ?? [], activeWorkspace?.id, matchingNoteIds(searchResults)),
    [snapshot, activeWorkspace?.id, searchResults],
  )
  // The one Thinking Graph projection. Degree, related Notes, relate
  // candidates, dimming, and the drawn graph are all read from this value, so
  // no two surfaces can count the same Relationship differently.
  const graph = useMemo(
    () => thinkingGraph(notes, snapshot?.relationships ?? []),
    [notes, snapshot?.relationships],
  )
  const focus = useNoteFocus(visible, graph)
  const localDiscovery = useLocalDiscovery(activeWorkspace)

  const cardContext: NoteCardContext = { graph, workspaces }

  // One set of Note intents, built once and handed to every card, so a layout
  // decides only where a Note appears and never what may be done to one.
  const noteIntents = buildNoteIntents({
    drafts,
    workspaces,
    submit,
    focusNote: focus.focusNote,
    startLabelRename: (label) => setRenameLabelDraft({ id: label.id, name: label.name }),
  })

  // The one card every view places, over the one set of intents.
  function noteCard(note: Note) {
    return (
      <NoteCard
        key={note.id}
        note={note}
        context={cardContext}
        drafts={drafts}
        intents={noteIntents}
        focused={focus.focusedNoteId === note.id}
        dimmed={focus.litNoteIds !== null && !focus.litNoteIds.has(note.id)}
        registerElement={(element) => focus.registerNoteElement(note.id, element)}
      />
    )
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

  function setAssistancePolicy(policy: AssistancePolicy) {
    if (!activeWorkspace) return
    void submit(thinkingWorkspace.setAssistancePolicy(activeWorkspace.id, policy))
  }

  function selectModel(modelId: string) {
    if (!activeWorkspace) return
    void submit(thinkingWorkspace.selectModel(activeWorkspace.id, modelId))
  }

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

      <AssistanceSection
        activeWorkspace={activeWorkspace}
        state={localDiscovery.state}
        query={localDiscovery.query}
        filteredModels={localDiscovery.filteredModels}
        selectedMissing={localDiscovery.selectedMissing}
        onPolicyChange={setAssistancePolicy}
        onQueryChange={localDiscovery.setQuery}
        onRefresh={localDiscovery.refresh}
        onSelectModel={selectModel}
      />

      <SearchSection
        query={searchQuery}
        searching={searchResults !== null}
        matchCount={visible.length}
        noteCount={notes.length}
        canSearch={Boolean(activeWorkspace)}
        onQueryChange={setSearchQuery}
        onSearch={search}
        onClear={() => { setSearchQuery(""); setSearchResults(null) }}
      />

      <CommittedNotesSection
        notes={visible}
        graph={graph}
        focus={focus}
        searching={searchResults !== null}
        view={view}
        canUndo={Boolean(snapshot) && snapshot!.undoableCommands > 0}
        onChooseView={setView}
        onUndo={undoLastChange}
        card={noteCard}
      />
      {renameLabelDraft && <section role="dialog" aria-label="Rename Label"><form onSubmit={saveRenamedLabel}><label htmlFor="rename-label">Label name</label><input autoFocus id="rename-label" value={renameLabelDraft.name} onChange={(event) => setRenameLabelDraft({ ...renameLabelDraft, name: event.target.value })} /><button type="submit">Save Label name</button><button type="button" onClick={() => setRenameLabelDraft(null)}>Cancel</button></form></section>}
    </main>
  )
}
