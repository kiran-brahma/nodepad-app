import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react"
import { ENRICH_DEBOUNCE_MILLIS } from "./enrichment-controller"
import type { Note, Relationship, WorkspaceOutcome, WorkspaceSnapshot } from "./workspace-client"
import type { EnrichmentCommandOutcome } from "./enrichment-contracts"

/**
 * R12 — Suggested Relationships, exercised at the one durable seam: the
 * `thinkingWorkspace` client over the Rust command surface. These tests prove
 * that a link the AI proposes arrives as a dashed line and a chip, that Link
 * is the only thing that commits one, that Dismiss commits nothing, and that
 * a waved-off pair is not offered again in the same session.
 */

const workspaceId = "workspace-1"
const firstNoteId = "note-1"
const secondNoteId = "note-2"
const firstText = "Cities grew around rivers"
const secondText = "Trade follows water"

let snapshot: WorkspaceSnapshot
/** Every `relate_notes` call, so committing a link is observable. */
let relateCalls: { noteId: string; otherNoteId: string }[] = []
/** Makes the next `relate_notes` refuse, so a failed commit is observable. */
let relateFails = false
let settleEnrichment: ((outcome: EnrichmentCommandOutcome) => void) | null = null

function seededNote(id: string, markdown: string): Note {
  return {
    id,
    workspaceId,
    markdown,
    noteType: "general",
    noteTypeProvenance: "default",
    annotation: null,
    annotationProvenance: "default",
    createdAt: `2026-07-27T10:0${id.slice(-1)}:00+00:00`,
    updatedAt: "2026-07-27T10:00:00+00:00",
    pinned: false,
    canvasX: 10,
    canvasY: 10,
    enrichmentRevision: 0,
    lastEnrichedAt: null,
    labels: [],
  }
}

function workspaceSnapshot(): WorkspaceSnapshot {
  return {
    workspaces: [
      {
        id: workspaceId,
        name: "Rivers",
        assistancePolicy: "local_ai",
        selectedModel: "phi3:latest",
        cloudConsentAt: null,
        createdAt: "2026-07-27T10:00:00+00:00",
        updatedAt: "2026-07-27T10:00:00+00:00",
      },
    ],
    notes: [seededNote(firstNoteId, firstText), seededNote(secondNoteId, secondText)],
    relationships: [],
    pendingSyntheses: [],
    activeWorkspaceId: workspaceId,
    undoableCommands: 0,
  }
}

function committed(): WorkspaceOutcome {
  return { status: "committed", snapshot }
}

/** What the Rust relate command commits: one Relationship for the pair. */
function relationship(noteIdA: string, noteIdB: string): Relationship {
  return {
    id: `relationship-${noteIdA}-${noteIdB}`,
    workspaceId,
    noteIdA,
    noteIdB,
    provenance: "manual",
    createdAt: "2026-07-27T11:00:00+00:00",
  }
}

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args: Record<string, unknown> = {}) => {
    switch (command) {
      case "get_workspace_snapshot":
        return Promise.resolve(committed())
      case "edit_note_text": {
        snapshot = {
          ...snapshot,
          notes: snapshot.notes.map((note) =>
            note.id === args.noteId
              ? {
                  ...note,
                  markdown: String(args.markdown),
                  enrichmentRevision: note.enrichmentRevision + 1,
                }
              : note,
          ),
          undoableCommands: 1,
        }
        return Promise.resolve(committed())
      }
      case "relate_notes": {
        relateCalls.push({
          noteId: String(args.noteId),
          otherNoteId: String(args.otherNoteId),
        })
        if (relateFails) {
          return Promise.resolve({
            status: "failed",
            failure: { code: "storage", message: "the Relationship was not written" },
          })
        }
        snapshot = {
          ...snapshot,
          relationships: [
            ...snapshot.relationships,
            relationship(String(args.noteId), String(args.otherNoteId)),
          ],
          undoableCommands: 1,
        }
        return Promise.resolve(committed())
      }
      case "enrich_note":
        return new Promise<EnrichmentCommandOutcome>((resolve) => {
          settleEnrichment = resolve
        })
      case "discover_local_models":
        return Promise.resolve({ status: "committed", models: ["phi3:latest"] })
      case "request_synthesis":
        return Promise.resolve({ status: "skipped", reason: "not eligible" })
      default:
        return Promise.resolve(committed())
    }
  },
}))

// `vi.mock` is hoisted above this import, so App sees the fake interface.
import { App } from "./App"

async function flush(): Promise<void> {
  await act(async () => {
    await Promise.resolve()
    await Promise.resolve()
  })
}

async function renderApp(): Promise<void> {
  snapshot = workspaceSnapshot()
  render(<App />)
  await flush()
}

/** Edits the first Note, which is what schedules an organization for it. */
async function editFirstNote(markdown: string): Promise<void> {
  fireEvent.click(screen.getAllByRole("button", { name: "Edit note text" })[0])
  const editor = screen.getByRole("textbox", { name: "Edit note text" })
  fireEvent.change(editor, { target: { value: markdown } })
  fireEvent.keyDown(editor, { key: "Enter" })
  await flush()
}

/** Runs the organization out and answers it with a proposed Relationship. */
async function proposeLink(relatedNoteIds: string[]): Promise<void> {
  await act(async () => {
    vi.advanceTimersByTime(ENRICH_DEBOUNCE_MILLIS)
    await Promise.resolve()
  })
  const resolve = settleEnrichment!
  settleEnrichment = null
  resolve({
    status: "applied",
    result: { noteType: "claim", labels: [], annotation: null, relatedNoteIds },
    snapshot,
  })
  await flush()
}

function dashedLines(): Element[] {
  return Array.from(document.querySelectorAll("line.canvas-relationship.suggested"))
}

function solidLines(): Element[] {
  return Array.from(document.querySelectorAll("line.canvas-relationship")).filter(
    (line) => !line.classList.contains("suggested"),
  )
}

beforeEach(() => {
  vi.useFakeTimers()
  relateCalls = []
  relateFails = false
  settleEnrichment = null
})

afterEach(() => {
  cleanup()
  vi.useRealTimers()
})

describe("suggested Relationships", () => {
  it("renders a proposed link as a dashed line and a chip, and commits nothing", async () => {
    await renderApp()
    await editFirstNote("Cities grew around rivers and harbours")
    await proposeLink([secondNoteId])

    expect(dashedLines()).toHaveLength(1)
    expect(solidLines()).toHaveLength(0)
    expect(screen.getByText(`Relate to “${secondText}”?`)).toBeTruthy()
    // A suggestion is an offer, never a commitment: nothing has been related.
    expect(relateCalls).toEqual([])
    expect(snapshot.relationships).toEqual([])
  })

  it("commits the link through the one relate command when the thinker links it", async () => {
    await renderApp()
    await editFirstNote("Cities grew around rivers and harbours")
    await proposeLink([secondNoteId])

    fireEvent.click(screen.getByRole("button", { name: `Link to ${secondText}` }))
    await flush()

    expect(relateCalls).toEqual([{ noteId: firstNoteId, otherNoteId: secondNoteId }])
    // An accepted suggestion is an ordinary Relationship afterwards: one solid
    // line, no dashed line, and no chip left to answer.
    expect(solidLines()).toHaveLength(1)
    expect(dashedLines()).toHaveLength(0)
    expect(screen.queryByText(`Relate to “${secondText}”?`)).toBeNull()
  })

  it("commits nothing when the thinker dismisses the suggestion", async () => {
    await renderApp()
    await editFirstNote("Cities grew around rivers and harbours")
    await proposeLink([secondNoteId])

    fireEvent.click(
      screen.getByRole("button", { name: `Dismiss suggested Relationship to ${secondText}` }),
    )
    await flush()

    expect(relateCalls).toEqual([])
    expect(dashedLines()).toHaveLength(0)
    expect(solidLines()).toHaveLength(0)
    expect(screen.queryByText(`Relate to “${secondText}”?`)).toBeNull()
  })

  it("does not offer a dismissed pair again while it is unchanged", async () => {
    await renderApp()
    const thought = "Cities grew around rivers and harbours"
    await editFirstNote(thought)
    await proposeLink([secondNoteId])
    fireEvent.click(
      screen.getByRole("button", { name: `Dismiss suggested Relationship to ${secondText}` }),
    )
    await flush()

    // The same pair, unchanged, proposed again by a later organization. The
    // thinker already answered this suggestion, so it is not put again.
    await editFirstNote(thought)
    await proposeLink([secondNoteId])

    expect(screen.queryByText(`Relate to “${secondText}”?`)).toBeNull()
    expect(dashedLines()).toHaveLength(0)
    expect(relateCalls).toEqual([])
  })

  it("may offer a dismissed pair again once the thought itself is rewritten", async () => {
    await renderApp()
    await editFirstNote("Cities grew around rivers and harbours")
    await proposeLink([secondNoteId])
    fireEvent.click(
      screen.getByRole("button", { name: `Dismiss suggested Relationship to ${secondText}` }),
    )
    await flush()

    // A rewritten Note is a different thought, so the pair the thinker waved
    // off is no longer the pair being proposed.
    await editFirstNote("Cities grew around rivers, harbours, and rail")
    await proposeLink([secondNoteId])

    expect(screen.getByText(`Relate to “${secondText}”?`)).toBeTruthy()
    expect(dashedLines()).toHaveLength(1)
    expect(relateCalls).toEqual([])
  })

  it("keeps the offer standing when the relate command refuses it", async () => {
    await renderApp()
    await editFirstNote("Cities grew around rivers and harbours")
    await proposeLink([secondNoteId])

    relateFails = true
    fireEvent.click(screen.getByRole("button", { name: `Link to ${secondText}` }))
    await flush()

    // Nothing was committed, so the suggestion is still unanswered: losing
    // the chip here would lose the proposal with no Relationship to show.
    expect(snapshot.relationships).toEqual([])
    expect(screen.getByText(`Relate to “${secondText}”?`)).toBeTruthy()
    expect(dashedLines()).toHaveLength(1)
  })
})
