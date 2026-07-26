import { cleanup, fireEvent, render, screen } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"
import { NoteCard, type NoteIntents } from "./note-card"
import { useNoteDrafts } from "./note-drafts"
import type { EnrichmentStatus } from "./enrichment-controller"
import type { Note } from "./workspace-client"

afterEach(cleanup)

const note: Note = {
  id: "note-1",
  workspaceId: "workspace-1",
  markdown: "A thought still being organized",
  noteType: "general",
  noteTypeProvenance: "default",
  annotation: null,
  annotationProvenance: "default",
  createdAt: "2026-07-26T00:00:00.000Z",
  updatedAt: "2026-07-26T00:00:00.000Z",
  pinned: false,
  canvasX: null,
  canvasY: null,
  labels: [],
  enrichmentRevision: 0,
  lastEnrichedAt: null,
}

function Card({ status, enabled = true, onRetry = vi.fn(), onReplace = vi.fn(), suggestedNotes = [], onAcceptSuggestion = vi.fn(), onDismissSuggestion = vi.fn() }: {
  status: EnrichmentStatus
  enabled?: boolean
  onRetry?: () => void
  onReplace?: () => void
  suggestedNotes?: Note[]
  onAcceptSuggestion?: (noteId: string) => void
  onDismissSuggestion?: (noteId: string) => void
}) {
  const drafts = useNoteDrafts()
  const intents: NoteIntents = {
    startEdit: () => {}, saveText: () => {}, cancelEdit: () => {},
    startAnnotation: () => {}, saveAnnotation: () => {}, cancelAnnotation: () => {},
    setNoteType: () => {}, togglePinned: () => {}, requestDelete: () => {}, answerDelete: () => {},
    startLabel: () => {}, editLabelDraft: () => {}, saveLabel: () => {}, cancelLabel: () => {},
    detachLabel: () => {}, startLabelRename: () => {}, removeLabel: () => {},
    startRelate: () => {}, editRelateQuery: () => {}, relate: () => {}, unrelate: () => {}, cancelRelate: () => {},
    focusNote: () => {}, startTransfer: () => {}, chooseTransferTarget: () => {}, transfer: () => {}, cancelTransfer: () => {},
    retryEnrichment: onRetry, requestReplaceEnrichment: onReplace,
    confirmReplaceEnrichment: () => {}, cancelReplaceEnrichment: () => {},
    editTextDraft: () => {}, editAnnotationDraft: () => {},
  }
  return <NoteCard note={note} context={{ graph: { nodes: [{ note, degree: 0 }], links: [] }, workspaces: [] }} drafts={drafts} intents={intents} focused={false} dimmed={false} registerElement={() => {}} enrichment={status} enrichmentEnabled={enabled} suggestedNotes={suggestedNotes} onAcceptSuggestion={onAcceptSuggestion} onDismissSuggestion={onDismissSuggestion} />
}

describe("Note-card enrichment presence", () => {
  it("shimmers while a Note is in flight without disabling its editor", () => {
    render(<Card status={{ kind: "in_flight", token: { workspaceId: "workspace-1", noteId: note.id, revision: 0, policy: "local_ai", endpoint: "http://localhost:11434", model: "phi3:latest" } }} />)
    expect(screen.getByRole("article").className).toContain("enrichment-active")
    expect(screen.getByRole("status").textContent).toContain("Organizing")
    expect(screen.getByRole("button", { name: "Edit note text" })).toBeDefined()
  })

  it("keeps stale recovery inline and delegates to the controller actions", () => {
    const onRetry = vi.fn()
    const onReplace = vi.fn()
    render(<Card status={{ kind: "failed", reason: "stale", message: "The Note changed." }} onRetry={onRetry} onReplace={onReplace} />)
    fireEvent.click(screen.getByRole("button", { name: "Retry" }))
    fireEvent.click(screen.getByRole("button", { name: "Re-enrich and Replace" }))
    expect(onRetry).toHaveBeenCalledOnce()
    expect(onReplace).toHaveBeenCalledOnce()
  })

  it("hides every enrichment signal when its Thinking Workspace is Manual", () => {
    render(<Card enabled={false} status={{ kind: "in_flight", token: { workspaceId: "workspace-1", noteId: note.id, revision: 0, policy: "local_ai", endpoint: "http://localhost:11434", model: "phi3:latest" } }} />)
    expect(screen.getByRole("article").className).not.toContain("enrichment-active")
    expect(screen.queryByText("Organizing…")).toBeNull()
  })

  it("keeps a suggested Relationship quiet until the thinker links or dismisses it", () => {
    const other = { ...note, id: "note-2", markdown: "A related thought" }
    const accept = vi.fn()
    const dismiss = vi.fn()
    render(<Card status={{ kind: "idle" }} suggestedNotes={[other]} onAcceptSuggestion={accept} onDismissSuggestion={dismiss} />)
    expect(screen.getByText("Relate to ‘A related thought’?"))
    fireEvent.click(screen.getByRole("button", { name: "Link" }))
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }))
    expect(accept).toHaveBeenCalledWith("note-2")
    expect(dismiss).toHaveBeenCalledWith("note-2")
  })
})
