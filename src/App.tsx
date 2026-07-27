import { FormEvent, useCallback, useEffect, useMemo, useState } from "react"
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
import { matchingNoteIds, noteViewLabel, NOTE_VIEWS, visibleNotes, workspaceNotes, type NoteView } from "./note-views"
import { NoteCard, type NoteCardContext } from "./note-card"
import { buildNoteIntents } from "./note-intents"
import { useNoteDrafts } from "./note-drafts"
import { useNoteFocus } from "./note-focus"
import { thinkingGraph } from "./thinking-graph"
import { useWorkspaceSnapshot } from "./workspace-snapshot"
import { useUndoShortcut } from "./undo-shortcut"
import { useEscape, ESCAPE_PRIORITY } from "./escape-stack"
import { useModalFocus } from "./modal-focus"
import { CommandPalette, useCommandPaletteShortcut, type PaletteAction } from "./command-palette"
import { WorkspaceSection } from "./workspace-section"
import { CaptureBar } from "./capture-bar"
import { SearchSection } from "./search-section"
import { CommittedNotesSection } from "./committed-notes-section"
import { StorageRecovery } from "./storage-recovery"
import { AppShell } from "./app-shell"
import { CloudConsentDialog } from "./assistance-section"
import { WorkspaceSettingsSheet } from "./workspace-settings-sheet"
import { IntroVideo } from "./intro-video"
import { useLocalDiscovery } from "./use-local-discovery"
import { useEnrichmentController } from "./enrichment-controller"
import { AiPresenceIndicator } from "./ai-presence"
import { useSynthesisController } from "./synthesis-controller"
import { SynthesisSection } from "./synthesis-section"
import { useCloudDiscovery } from "./use-cloud-discovery"
import { useSuggestedRelationships } from "./suggested-relationships"

// App is the V0 orchestrator: a pre-existing 400-line component that wires
// every section to the one durable seam. The Command-K palette adds exactly
// one conditional render (`paletteOpen && …`); decomposing App into smaller
// screens is out of scope for this interaction slice and would cross the V0-17
// scope fence. Tracked here so the threshold stays honest instead of hidden.
// fallow-ignore-next-line complexity
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
  // The Cloud AI disclosure. Visible only while the active Workspace has not
  // given consent and the thinker has asked to use Cloud AI. Recording
  // consent is what flips the policy to cloud_ai; nothing else does.
  const [consentDialog, setConsentDialog] = useState<{ workspaceId: string; workspaceName: string; provider: import("./workspace-client").CloudProvider } | null>(null)
  // Command-K opens the palette; it owns no business rule, only an open flag.
  const [paletteOpen, setPaletteOpen] = useState(false)
  // The Workspace settings sheet. Opens from the rail; closes on Escape or
  // scrim click. All settings controls are inside the sheet, not the main pane.
  const [settingsOpen, setSettingsOpen] = useState(false)

  const activeWorkspace = useMemo(
    () => snapshot?.workspaces.find(({ id }) => id === snapshot.activeWorkspaceId),
    [snapshot],
  )
  // How the same committed Notes are arranged. Not committed, so a restart
  // reconstructs the default view from SQLite alone. The choice persists per
  // active Workspace as transient UI state (not written to the snapshot).
  const [viewByWorkspace, setViewByWorkspace] = useState<Map<string, NoteView>>(new Map())
  const view = viewByWorkspace.get(activeWorkspace?.id ?? "") ?? "canvas"
  // Crossfade transition key: increments on view change to trigger a brief
  // CSS fade-in animation on the view container. The existing global
  // prefers-reduced-motion rule neutralises all animations automatically.
  const [transitionKey, setTransitionKey] = useState(0)

  function chooseView(nextView: NoteView) {
    if (nextView === view || !activeWorkspace) return
    setViewByWorkspace((prev) => {
      const next = new Map(prev)
      next.set(activeWorkspace.id, nextView)
      return next
    })
    setTransitionKey((k) => k + 1)
  }
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

  // AI-proposed Relationships wait here until the thinker answers them. The
  // holder keeps only what the Thinking Graph cannot say — which pairs were
  // proposed, and which were waved off — and accepting commits through the
  // one relate command, so an accepted suggestion is an ordinary Relationship.
  const suggestions = useSuggestedRelationships(
    graph,
    useCallback(
      (noteId: string, otherNoteId: string) =>
        submit(thinkingWorkspace.relateNotes(noteId, otherNoteId)).then(
          (result) => result.committed,
        ),
      [submit],
    ),
  )

  // The one place a proposal enters the session. The controller's `applied`
  // status carries the result the model returned; the Relationships in it are
  // proposals, because the durable layer commits none of them.
  const enrichmentStatus = enrichment.status
  const activeEnrichedNoteId = enrichment.activeNoteId
  const proposeSuggestions = suggestions.propose
  useEffect(() => {
    if (enrichmentStatus.kind !== "applied") return
    if (!activeEnrichedNoteId) return
    proposeSuggestions(activeEnrichedNoteId, enrichmentStatus.result.relatedNoteIds)
  }, [enrichmentStatus, activeEnrichedNoteId, proposeSuggestions])

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
    onDismissEnrichment: () => enrichment.clear(),
    onAcceptSuggestion: suggestions.accept,
    onDismissSuggestion: suggestions.dismiss,
  })

  // The one card every view places, over the one set of intents.
  function noteCard(note: Note) {
    // The one gate on per-Note AI presence, the same predicate the top-bar
    // indicator reads. A Workspace switched to Manual mid-flight loses its
    // shimmer with the indicator, never after it.
    const cardEnrichment =
      aiEnabled && enrichment.activeNoteId === note.id ? enrichment.status : undefined
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
        enrichment={cardEnrichment}
        suggestions={suggestions.forNote(note.id)}
      />
    )
  }

  /** Reports whether the create committed, so the rail's inline field closes
   *  on a commit and keeps the name when the command refuses it. */
  function createWorkspace(): Promise<boolean> {
    return submit(thinkingWorkspace.createWorkspace(workspaceName)).then((result) => {
      if (result.committed) setWorkspaceName("")
      return result.committed
    })
  }

  /** The one switch. The rail row and the ⌘K jump entries both run it, so
   *  which Workspace is active is always a fact the next snapshot reports. */
  function selectWorkspace(workspaceId: string) {
    void submit(thinkingWorkspace.selectWorkspace(workspaceId))
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
    if (noteMarkdown.trim() === "") return
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
      setConsentDialog({ workspaceId: activeWorkspace.id, workspaceName: activeWorkspace.name, provider: activeWorkspace.cloudProvider ?? "ollama" })
      return
    }
    void submit(thinkingWorkspace.setAssistancePolicy(activeWorkspace.id, policy))
  }

  function selectModel(modelId: string) {
    if (!activeWorkspace) return
    void submit(thinkingWorkspace.selectModel(activeWorkspace.id, modelId))
  }

  function setCloudProvider(provider: import("./workspace-client").CloudProvider) {
    if (!activeWorkspace) return
    void submit(thinkingWorkspace.setCloudProvider(activeWorkspace.id, provider))
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
  useCommandPaletteShortcut(setPaletteOpen)

  const canUndo = Boolean(snapshot) && snapshot!.undoableCommands > 0
  const paletteActions = buildPaletteActions({
    activeWorkspace,
    workspaces,
    selectWorkspace,
    canUndo,
    undo: undoLastChange,
    renameWorkspace: () => setRenameDraft({ id: activeWorkspace!.id, name: activeWorkspace!.name }),
    deleteWorkspace: () => setPendingDelete(requestDelete(activeWorkspace!)),
    exportMarkdown: exportWorkspace,
    exportArchive: exportWorkspaceArchive,
    importArchive: importWorkspaceArchive,
    setView: chooseView,
    setAssistancePolicy,
  })

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
    <>
      <AppShell
      rail={
        <WorkspaceSection
          workspaces={workspaces}
          activeWorkspaceId={activeWorkspace?.id}
          name={workspaceName}
          renameDraft={renameDraft}
          onSelect={selectWorkspace}
          onNameChange={setWorkspaceName}
          onCreate={createWorkspace}
          onStartRename={(workspace) => setRenameDraft({ id: workspace.id, name: workspace.name })}
          onRenameDraftChange={(name) => setRenameDraft((draft) => (draft ? { ...draft, name } : draft))}
          onRename={renameWorkspace}
          onCancelRename={() => setRenameDraft(null)}
          onOpenSettings={() => setSettingsOpen(true)}
        />
      }
      topbar={
        <>
          <div className="seg" role="group" aria-label="Note view">
            {NOTE_VIEWS.map((option) => (
              <button
                key={option}
                aria-pressed={view === option}
                className={view === option ? "active" : ""}
                onClick={() => chooseView(option)}
              >
                {noteViewLabel(option)}
              </button>
            ))}
          </div>
          {/* The only AI signal in the main view besides the per-Note
              shimmer. No AI configuration lives here; that is in the
              Workspace settings sheet. */}
          <AiPresenceIndicator enabled={aiEnabled} status={enrichment.status} />
        </>
      }
      main={
        <>
          <header>
            <p className="eyebrow">Nodepad</p>
            <h1>Thinking Workspace</h1>
            <p>Capture one atomic thought at a time. Every change is committed locally before it appears here.</p>
          </header>

          <IntroVideo />

          {failure && <aside role="alert">{failure.message} <button onClick={dismissFailure}>Dismiss</button></aside>}

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

          <div key={transitionKey} className="view-fade">
            <CommittedNotesSection
              notes={visible}
              graph={graph}
              focus={focus}
              searching={searchResults !== null}
              view={view}
              canUndo={canUndo}
              onUndo={undoLastChange}
              onSetPosition={(noteId, x, y) => submit(thinkingWorkspace.setNotePosition(noteId, x, y))}
              onSetNoteType={noteIntents.setNoteType}
              onRelate={(noteId, otherNoteId) => submit(thinkingWorkspace.relateNotes(noteId, otherNoteId))}
              onUnrelate={(noteId, otherNoteId) => submit(thinkingWorkspace.unrelateNotes(noteId, otherNoteId))}
              card={noteCard}
              pendingSyntheses={synthesis.pending}
              suggestions={suggestions.visible}
              onAcceptSynthesis={synthesis.accept}
              onDismissSynthesis={synthesis.dismiss}
            />
          </div>

          <SynthesisSection
            pending={synthesis.pending}
            notes={notes}
            status={synthesis.status}
            aiEnabled={aiEnabled}
            onAccept={synthesis.accept}
            onDismiss={synthesis.dismiss}
          />
        </>
      }
      footer={
        <CaptureBar
          activeWorkspace={activeWorkspace}
          noteMarkdown={noteMarkdown}
          onNoteMarkdownChange={setNoteMarkdown}
          onCreateNote={createNote}
        />
      }
    />

      {settingsOpen && activeWorkspace && (
        <WorkspaceSettingsSheet
          activeWorkspace={activeWorkspace}
          renameDraft={renameDraft}
          pendingDelete={pendingDelete}
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
          /* Closing the sheet abandons an uncommitted rename with it, so the
             one draft never outlives the surface the thinker dismissed and
             lingers as an open field on the rail row. */
          onClose={() => {
            setSettingsOpen(false)
            setRenameDraft(null)
          }}
          onStartRename={(workspace: ThinkingWorkspace) =>
            setRenameDraft({ id: workspace.id, name: workspace.name })
          }
          onRenameDraftChange={(name) => setRenameDraft((draft) => (draft ? { ...draft, name } : draft))}
          onRename={renameWorkspace}
          onCancelRename={() => setRenameDraft(null)}
          onRequestDelete={(workspace) => setPendingDelete(requestDelete(workspace))}
          onAnswerDelete={answerDeleteConfirmation}
          onExport={exportWorkspace}
          onExportArchive={exportWorkspaceArchive}
          onImportArchive={importWorkspaceArchive}
          onPolicyChange={setAssistancePolicy}
          onCloudProviderChange={setCloudProvider}
          onLocalQueryChange={localDiscovery.setQuery}
          onLocalRefresh={localDiscovery.refresh}
          onCloudQueryChange={cloudDiscovery.setQuery}
          onCloudRefresh={cloudDiscovery.refresh}
          onCloudKeyChange={cloudDiscovery.refreshKeyPresence}
          onRequestCloudConsent={() =>
            activeWorkspace &&
            setConsentDialog({ workspaceId: activeWorkspace.id, workspaceName: activeWorkspace.name, provider: activeWorkspace.cloudProvider ?? "ollama" })
          }
          onRevokeCloudConsent={revokeCloudConsent}
          onSelectModel={selectModel}
        />
      )}

      {renameLabelDraft && (
        <RenameLabelModal
          draft={renameLabelDraft}
          onDraftChange={(name) => setRenameLabelDraft({ ...renameLabelDraft, name })}
          onSubmit={saveRenamedLabel}
          onClose={() => setRenameLabelDraft(null)}
        />
      )}

      {consentDialog && (
        <CloudConsentDialog
          workspaceId={consentDialog.workspaceId}
          workspaceName={consentDialog.workspaceName}
          provider={consentDialog.provider}
          onAccepted={handleConsentAccepted}
          onClose={() => setConsentDialog(null)}
        />
      )}

      {paletteOpen && (
        <CommandPalette
          onClose={() => setPaletteOpen(false)}
          actions={paletteActions}
        />
      )}
    </>
  )
}

/** A true modal for renaming a Label: focus is trapped and restored to the
 *  control that opened it, Escape cancels, and a click on the scrim cancels. */
function RenameLabelModal({
  draft,
  onDraftChange,
  onSubmit,
  onClose,
}: {
  draft: { id: string; name: string }
  onDraftChange: (name: string) => void
  onSubmit: (event: import("react").FormEvent) => void
  onClose: () => void
}) {
  const ref = useModalFocus<HTMLDivElement>(true)
  useEscape(onClose, ESCAPE_PRIORITY.modal)
  return (
    <div className="modal-overlay" onMouseDown={(event) => {
      if (event.target === event.currentTarget) onClose()
    }}>
      <section ref={ref} className="modal" role="dialog" aria-modal="true" aria-label="Rename Label">
        <form onSubmit={onSubmit}>
          <label htmlFor="rename-label">Label name</label>
          <input id="rename-label" value={draft.name} onChange={(event) => onDraftChange(event.target.value)} />
          <button type="submit">Save Label name</button>
          <button type="button" onClick={onClose}>Cancel</button>
        </form>
      </section>
    </div>
  )
}

/** Builds the Command-K palette's actions from handlers that already exist in
 *  App, so the palette module itself never learns about Workspaces, views, or
 *  assistance policy. A module-level helper keeps the branching out of the
 *  App component body. Returns nothing when there is no active Workspace. */
function buildPaletteActions(input: {
  activeWorkspace: ThinkingWorkspace | undefined
  /** Every Thinking Workspace, so ⌘K can jump to one by name. */
  workspaces: ThinkingWorkspace[]
  selectWorkspace: (workspaceId: string) => void
  canUndo: boolean
  undo: () => void
  renameWorkspace: () => void
  deleteWorkspace: () => void
  exportMarkdown: () => void
  exportArchive: () => void
  importArchive: () => void
  setView: (view: NoteView) => void
  setAssistancePolicy: (policy: AssistancePolicy) => void
}): PaletteAction[] {
  if (!input.activeWorkspace) return []
  return [
    { id: "new-note", label: "New Note", group: "Notes", run: () => document.getElementById("note")?.focus() },
    { id: "undo", label: "Undo", group: "Notes", disabled: !input.canUndo, run: input.undo },
    { id: "rename-workspace", label: "Rename Workspace", group: "Workspace", run: input.renameWorkspace },
    { id: "delete-workspace", label: "Delete Workspace", group: "Workspace", run: input.deleteWorkspace },
    { id: "export-markdown", label: "Export Markdown", group: "Workspace", run: input.exportMarkdown },
    { id: "export-archive", label: "Export Archive", group: "Workspace", run: input.exportArchive },
    { id: "import-archive", label: "Import Archive", group: "Workspace", run: input.importArchive },
    { id: "view-canvas", label: "Canvas view", group: "View", run: () => input.setView("canvas") },
    { id: "view-kanban", label: "Kanban view", group: "View", run: () => input.setView("kanban") },
    { id: "view-graph", label: "Graph view", group: "View", run: () => input.setView("graph") },
    { id: "policy-manual", label: "Assistance: Manual", group: "Assistance", run: () => input.setAssistancePolicy("manual") },
    { id: "policy-local", label: "Assistance: Local AI", group: "Assistance", run: () => input.setAssistancePolicy("local_ai") },
    { id: "policy-cloud", label: "Assistance: Cloud AI", group: "Assistance", run: () => input.setAssistancePolicy("cloud_ai") },
    // One jump per Thinking Workspace, matched by name, running the same
    // switch the rail row runs. Full palette coverage is a later slice.
    ...input.workspaces.map((workspace) => ({
      id: `switch-workspace-${workspace.id}`,
      label: `Switch Workspace → ${workspace.name}`,
      group: "Switch Workspace",
      run: () => input.selectWorkspace(workspace.id),
    })),
  ]
}
