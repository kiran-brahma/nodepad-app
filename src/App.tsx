import { FormEvent, useCallback, useMemo, useState } from "react"
import {
  assistanceEnabled,
  thinkingWorkspace,
  type AssistancePolicy,
  type Note,
  type SearchResult,
  type ThinkingWorkspace,
  type WorkspaceOutcome,
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
import { AssistanceSection, CloudConsentDialog } from "./assistance-section"
import { useLocalDiscovery } from "./use-local-discovery"
import { useEnrichmentController } from "./enrichment-controller"
import { useSynthesisController } from "./synthesis-controller"
import { SynthesisSection } from "./synthesis-section"
import { useCloudDiscovery } from "./use-cloud-discovery"

export function App() {
  const { snapshot, openFailure, failure, submit, adoptSnapshot, recoverWithSnapshot, reportFailure, dismissFailure } =
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
  // The Cloud AI disclosure. Visible only while the active Workspace has not
  // given consent and the thinker has asked to use Cloud AI. Recording
  // consent is what flips the policy to cloud_ai; nothing else does.
  const [consentDialog, setConsentDialog] = useState<{ workspaceId: string; workspaceName: string } | null>(null)

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
  const cloudDiscovery = useCloudDiscovery(activeWorkspace)
  // Whether this Workspace may make an AI call at all. Manual Workspaces
  // never enrich a Note and never request a Synthesis.
  const aiEnabled = assistanceEnabled(activeWorkspace)
  const enrichment = useEnrichmentController({
    workspaceId: activeWorkspace?.id ?? "",
    snapshot: snapshot ?? null,
    enabled: aiEnabled,
  })

  // Synthesis eligibility, the cooldown, and the pending cap are decided in
  // Rust against durable state; the controller only schedules the attempt
  // and reports what came back.
  const synthesis = useSynthesisController({
    workspaceId: activeWorkspace?.id ?? "",
    snapshot: snapshot ?? null,
    enabled: aiEnabled,
    onSnapshot: adoptSnapshot,
    submit,
  })

  const cardContext: NoteCardContext = { graph, workspaces }

  // One set of Note intents, built once and handed to every card, so a layout
  // decides only where a Note appears and never what may be done to one.
  const noteIntents = buildNoteIntents({
    drafts,
    workspaces,
    submit,
    focusNote: focus.focusNote,
    startLabelRename: (label) => setRenameLabelDraft({ id: label.id, name: label.name }),
    onNoteTextSaved: (noteId) => {
      enrichment.schedule(noteId)
      // Editing a Note changes the material a Synthesis would rest on, so
      // the next attempt is scheduled here too. Rust refuses it unless the
      // Workspace has actually grown and the cooldown has passed.
      synthesis.schedule()
    },
    onRetryEnrichment: () => enrichment.retry(),
    onRequestReplaceEnrichment: () => enrichment.requestReplace(),
    onConfirmReplaceEnrichment: () => enrichment.confirmReplace(),
    onCancelReplaceEnrichment: () => enrichment.cancelReplace(),
  })

  // The one card every view places, over the one set of intents.
  function noteCard(note: Note) {
    const enrichmentStatus =
      enrichment.activeNoteId === note.id ? enrichment.status : undefined
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
        enrichment={enrichmentStatus}
      />
    )
  }

  function createWorkspace(event: FormEvent) {
    event.preventDefault()
    void submit(thinkingWorkspace.createWorkspace(workspaceName)).then((result) => {
      if (result.committed) setWorkspaceName("")
    })
  }

  function renameWorkspace(event: FormEvent) {
    event.preventDefault()
    if (!renameDraft) return
    void submit(thinkingWorkspace.renameWorkspace(renameDraft.id, renameDraft.name)).then(
      (result) => {
        if (result.committed) setRenameDraft(null)
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
    void submit(thinkingWorkspace.createNote(activeWorkspace.id, noteMarkdown)).then((result) => {
      if (!result.committed || !result.snapshot) return
      setNoteMarkdown("")
      // Find the freshly committed Note so the Enrichment controller can
      // schedule an automatic organization attempt for it.
      const newest = [...result.snapshot.notes]
        .filter((candidate) => candidate.workspaceId === activeWorkspace.id)
        .sort((left, right) => right.createdAt.localeCompare(left.createdAt))[0]
      if (newest) enrichment.schedule(newest.id)
      synthesis.schedule()
    })
  }

  function exportWorkspace() {
    if (!activeWorkspace) return
    void thinkingWorkspace.exportWorkspace(activeWorkspace.id).then((outcome) => {
      if (outcome.status === "failed") reportFailure({ code: "storage", message: outcome.message })
    })
  }

  function exportWorkspaceArchive() {
    if (!activeWorkspace) return
    void thinkingWorkspace.exportWorkspaceArchive(activeWorkspace.id).then((outcome) => {
      if (outcome.status === "failed") reportFailure({ code: "storage", message: outcome.message })
    })
  }

  function importWorkspaceArchive() {
    void thinkingWorkspace.importWorkspaceArchive().then((outcome) => {
      if (outcome.status === "imported") adoptSnapshot(outcome.snapshot)
      else if (outcome.status === "failed")
        reportFailure({ code: "storage", message: outcome.message })
    })
  }

  function saveRenamedLabel(event: FormEvent) {
    event.preventDefault()
    if (!renameLabelDraft) return
    void submit(thinkingWorkspace.renameLabel(renameLabelDraft.id, renameLabelDraft.name)).then((result) => {
      if (result.committed) setRenameLabelDraft(null)
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

  /**
   * Switching the Assistance Policy. Selecting Cloud AI without consent
   * opens the disclosure instead of writing the policy; the disclosure
   * commit is the only path that lands the policy on cloud_ai.
   */
  function setAssistancePolicy(policy: AssistancePolicy) {
    if (!activeWorkspace) return
    if (policy === "cloud_ai" && activeWorkspace.cloudConsentAt === null) {
      setConsentDialog({ workspaceId: activeWorkspace.id, workspaceName: activeWorkspace.name })
      return
    }
    void submit(thinkingWorkspace.setAssistancePolicy(activeWorkspace.id, policy))
  }

  function selectModel(modelId: string) {
    if (!activeWorkspace) return
    void submit(thinkingWorkspace.selectModel(activeWorkspace.id, modelId))
  }

  function handleConsentAccepted(outcome: WorkspaceOutcome) {
    if (outcome.status !== "committed") {
      setConsentDialog(null)
      return
    }
    const workspaceId = consentDialog?.workspaceId
    setConsentDialog(null)
    if (!workspaceId) return
    // The disclosure records consent; this second call moves the policy
    // onto Cloud AI. The two commits are intentionally separate, so a
    // failure on one leaves the other durable.
    void submit(thinkingWorkspace.setAssistancePolicy(workspaceId, "cloud_ai"))
  }

  function revokeCloudConsent() {
    if (!activeWorkspace) return
    // Revoking consent returns the Workspace to Manual so the durable
    // policy can never read "cloud_ai" while the Workspace is not consented.
    void submit(thinkingWorkspace.setCloudConsent(activeWorkspace.id, false))
    void submit(thinkingWorkspace.setAssistancePolicy(activeWorkspace.id, "manual"))
  }

  useUndoShortcut(undoLastChange)

  if (openFailure) {
    return (
      <StorageRecovery
        failure={openFailure}
        onRetry={() => void submit(thinkingWorkspace.retryStorageOpen())}
        onQuit={() => void thinkingWorkspace.quitApplication()}
        onRestored={(snapshot) => {
          recoverWithSnapshot(snapshot)
        }}
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
        onExport={exportWorkspace}
        onExportArchive={exportWorkspaceArchive}
        onImportArchive={importWorkspaceArchive}
      />

      <AssistanceSection
        activeWorkspace={activeWorkspace}
        localState={localDiscovery.state}
        localQuery={localDiscovery.query}
        localFilteredModels={localDiscovery.filteredModels}
        cloudState={cloudDiscovery.state}
        cloudQuery={cloudDiscovery.query}
        cloudFilteredModels={cloudDiscovery.filteredModels}
        cloudKeyPresent={cloudDiscovery.keyPresent}
        selectedMissing={
          localDiscovery.selectedMissing || cloudDiscovery.selectedMissing
        }
        onPolicyChange={setAssistancePolicy}
        onLocalQueryChange={localDiscovery.setQuery}
        onLocalRefresh={localDiscovery.refresh}
        onCloudQueryChange={cloudDiscovery.setQuery}
        onCloudRefresh={cloudDiscovery.refresh}
        onCloudKeyChange={cloudDiscovery.refreshKeyPresence}
        onRequestCloudConsent={() =>
          activeWorkspace &&
          setConsentDialog({ workspaceId: activeWorkspace.id, workspaceName: activeWorkspace.name })
        }
        onRevokeCloudConsent={revokeCloudConsent}
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
        pendingSyntheses={synthesis.pending}
      />

      <SynthesisSection
        pending={synthesis.pending}
        notes={notes}
        status={synthesis.status}
        aiEnabled={aiEnabled}
        onAccept={synthesis.accept}
        onDismiss={synthesis.dismiss}
      />
      {renameLabelDraft && <section role="dialog" aria-label="Rename Label"><form onSubmit={saveRenamedLabel}><label htmlFor="rename-label">Label name</label><input autoFocus id="rename-label" value={renameLabelDraft.name} onChange={(event) => setRenameLabelDraft({ ...renameLabelDraft, name: event.target.value })} /><button type="submit">Save Label name</button><button type="button" onClick={() => setRenameLabelDraft(null)}>Cancel</button></form></section>}

      {consentDialog && (
        <CloudConsentDialog
          workspaceId={consentDialog.workspaceId}
          workspaceName={consentDialog.workspaceName}
          onAccepted={handleConsentAccepted}
          onClose={() => setConsentDialog(null)}
        />
      )}
    </main>
  )
}
