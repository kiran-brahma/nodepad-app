import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react"
import { ENRICH_DEBOUNCE_MILLIS } from "./enrichment-controller"
import type {
  AssistancePolicy,
  Note,
  WorkspaceOutcome,
  WorkspaceSnapshot,
} from "./workspace-client"
import type { EnrichmentCommandOutcome } from "./enrichment-contracts"

/**
 * R11 — Background enrichment presence, exercised at the one durable seam:
 * the `thinkingWorkspace` client over the Rust command surface. These tests
 * prove what the thinker sees while AI organizes a Note — a shimmer on the
 * Note and one quiet top-bar indicator — that editing is never blocked, and
 * that a Manual Workspace shows no AI presence at all.
 */

const workspaceId = "workspace-1"
const noteId = "note-1"
/** The Note the capture bar commits, which only exists after `create_note`. */
const capturedNoteId = "note-2"
const noteText = "Cities grew around rivers"
let snapshot: WorkspaceSnapshot
/** Every `enrich_note` call, so a Retry or a Replace is observable. */
let enrichCalls: { noteId: string; force: boolean }[] = []
/** Resolves the in-flight `enrich_note`, so `in_flight` can be held open. */
let settleEnrichment: ((outcome: EnrichmentCommandOutcome) => void) | null = null

function seededNote(): Note {
  return {
    id: noteId,
    workspaceId,
    markdown: noteText,
    noteType: "general",
    noteTypeProvenance: "default",
    annotation: null,
    annotationProvenance: "default",
    createdAt: "2026-07-22T10:00:00+00:00",
    updatedAt: "2026-07-22T10:00:00+00:00",
    pinned: false,
    canvasX: null,
    canvasY: null,
    enrichmentRevision: 0,
    lastEnrichedAt: null,
    labels: [],
  }
}

function workspaceSnapshot(policy: AssistancePolicy): WorkspaceSnapshot {
  return {
    workspaces: [
      {
        id: workspaceId,
        name: "Rivers",
        assistancePolicy: policy,
        selectedModel: "phi3:latest",
        cloudConsentAt: null,
        createdAt: "2026-07-22T10:00:00+00:00",
        updatedAt: "2026-07-22T10:00:00+00:00",
      },
    ],
    notes: [seededNote()],
    relationships: [],
    pendingSyntheses: [],
    activeWorkspaceId: workspaceId,
    undoableCommands: 0,
  }
}

function committed(): WorkspaceOutcome {
  return { status: "committed", snapshot }
}

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args: Record<string, unknown> = {}) => {
    switch (command) {
      case "get_workspace_snapshot":
        return Promise.resolve(committed())
      case "create_note": {
        snapshot = {
          ...snapshot,
          notes: [
            ...snapshot.notes,
            {
              ...seededNote(),
              id: capturedNoteId,
              markdown: String(args.markdown),
              createdAt: "2026-07-22T11:00:00+00:00",
              updatedAt: "2026-07-22T11:00:00+00:00",
            },
          ],
          undoableCommands: 1,
        }
        return Promise.resolve(committed())
      }
      case "edit_note_text": {
        // Editing bumps the durable revision, which is what makes an
        // in-flight response stale on the Rust side.
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
      case "enrich_note":
        enrichCalls.push({ noteId: String(args.noteId), force: Boolean(args.force) })
        return new Promise<EnrichmentCommandOutcome>((resolve) => {
          settleEnrichment = resolve
        })
      case "set_assistance_policy": {
        const policy = args.policy as AssistancePolicy
        snapshot = {
          ...snapshot,
          workspaces: snapshot.workspaces.map((workspace) =>
            workspace.id === args.workspaceId
              ? { ...workspace, assistancePolicy: policy }
              : workspace,
          ),
        }
        return Promise.resolve(committed())
      }
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

/** Lets pending promises settle inside React's act boundary. */
async function flush(): Promise<void> {
  await act(async () => {
    await Promise.resolve()
    await Promise.resolve()
  })
}

/** Edits the seeded Note's text, which is what schedules an organization. */
async function editNoteText(markdown: string): Promise<void> {
  fireEvent.click(screen.getByRole("button", { name: "Edit note text" }))
  const editor = screen.getByRole("textbox", { name: "Edit note text" })
  fireEvent.change(editor, { target: { value: markdown } })
  fireEvent.keyDown(editor, { key: "Enter" })
  await flush()
}

/** Captures a fresh Note through the capture bar, the other path that
 *  schedules an organization. */
async function captureNote(markdown: string): Promise<void> {
  const capture = screen.getByRole("textbox", { name: "New Note" })
  fireEvent.change(capture, { target: { value: markdown } })
  fireEvent.keyDown(capture, { key: "Enter" })
  await flush()
}

/** Runs out the debounce window, so the attempt reaches `in_flight`. */
async function reachInFlight(): Promise<void> {
  await act(async () => {
    vi.advanceTimersByTime(ENRICH_DEBOUNCE_MILLIS)
    await Promise.resolve()
  })
}

async function settle(outcome: EnrichmentCommandOutcome): Promise<void> {
  const resolve = settleEnrichment!
  settleEnrichment = null
  resolve(outcome)
  await flush()
}

function card(): HTMLElement {
  return screen.getAllByRole("article")[0]
}

/** The Note reads as being worked on: the shimmer and the `aria-busy` flag
 *  are the same fact, so both are checked together. */
function shimmering(): boolean {
  const element = card()
  return element.className.includes("organizing") && element.getAttribute("aria-busy") === "true"
}

async function renderApp(policy: AssistancePolicy): Promise<void> {
  snapshot = workspaceSnapshot(policy)
  render(<App />)
  await flush()
}

beforeEach(() => {
  vi.useFakeTimers()
  enrichCalls = []
  settleEnrichment = null
})

afterEach(() => {
  cleanup()
  vi.useRealTimers()
})

describe("background enrichment presence", () => {
  it("shimmers the Note and reads 'AI · working' while it is being organized", async () => {
    await renderApp("local_ai")
    expect(screen.getByText("AI · quiet")).toBeTruthy()
    expect(shimmering()).toBe(false)

    await editNoteText("Cities grew around rivers and harbours")
    await reachInFlight()

    expect(enrichCalls).toEqual([{ noteId, force: false }])
    expect(shimmering()).toBe(true)
    expect(screen.getByText("AI · working")).toBeTruthy()
    expect(screen.queryByText("AI · quiet")).toBeNull()
  })

  it("organizes a freshly captured Note, not only an edited one", async () => {
    await renderApp("local_ai")
    await captureNote("Harbours followed the deltas")
    await reachInFlight()

    // The attempt is scheduled from the commit callback, one render before the
    // new Note is in the snapshot, so the token must be resolved against
    // current state rather than the state captured at schedule time.
    expect(enrichCalls).toEqual([{ noteId: capturedNoteId, force: false }])
  })

  it("drops the shimmer and returns to quiet once the organization is applied", async () => {
    await renderApp("local_ai")
    await editNoteText("Trade follows water")
    await reachInFlight()

    await settle({
      status: "applied",
      result: { noteType: "claim", labels: [], annotation: null, relatedNoteIds: [] },
      snapshot,
    })

    expect(shimmering()).toBe(false)
    expect(screen.getByText("AI · quiet")).toBeTruthy()
  })

  it("keeps the Note editable while it is in flight", async () => {
    await renderApp("local_ai")
    await editNoteText("Trade follows water")
    await reachInFlight()
    expect(shimmering()).toBe(true)

    // No overlay, no disabled control: the thought opens for editing and the
    // edit commits while the request is still out.
    fireEvent.click(screen.getByRole("button", { name: "Edit note text" }))
    const editor = screen.getByRole("textbox", { name: "Edit note text" })
    fireEvent.change(editor, { target: { value: "Trade follows water and rail" } })
    fireEvent.keyDown(editor, { key: "Enter" })
    await flush()

    expect(screen.getByText("Trade follows water and rail")).toBeTruthy()
    // The edit bumped the durable revision the in-flight token captured, which
    // is the existing guard this slice relies on rather than replacing.
    expect(snapshot.notes[0].enrichmentRevision).toBe(2)
  })

  it("shows no enrichment presence at all in a Manual Workspace", async () => {
    await renderApp("manual")
    await editNoteText("A manual thought")
    await reachInFlight()

    expect(screen.queryByText("AI · quiet")).toBeNull()
    expect(screen.queryByText("AI · working")).toBeNull()
    expect(shimmering()).toBe(false)
    expect(enrichCalls).toEqual([])
  })

  it("drops the shimmer with the indicator when the policy turns Manual mid-flight", async () => {
    await renderApp("local_ai")
    await editNoteText("A thought under review")
    await reachInFlight()
    expect(shimmering()).toBe(true)

    fireEvent.click(screen.getByRole("button", { name: "Workspace settings" }))
    fireEvent.click(screen.getByRole("button", { name: "Manual" }))
    await flush()

    // Presence is gated by one predicate, so the Note cannot keep shimmering
    // in a Workspace whose top bar has already gone silent.
    expect(shimmering()).toBe(false)
    expect(screen.queryByText("Organizing…")).toBeNull()
    expect(screen.queryByText("AI · working")).toBeNull()
    expect(screen.queryByText("AI · quiet")).toBeNull()
  })

  it("offers inline retry and dismiss when an organization fails", async () => {
    await renderApp("local_ai")
    await editNoteText("A failing thought")
    await reachInFlight()
    await settle({
      status: "provider_failed",
      code: "unavailable",
      message: "the model is not reachable",
      snapshot,
    })

    expect(screen.getByText("AI request failed")).toBeTruthy()
    expect(screen.queryByRole("button", { name: "Re-enrich and Replace" })).toBeNull()

    fireEvent.click(screen.getByRole("button", { name: "Retry" }))
    await flush()
    expect(enrichCalls).toEqual([
      { noteId, force: false },
      { noteId, force: false },
    ])

    await settle({
      status: "provider_failed",
      code: "unavailable",
      message: "the model is not reachable",
      snapshot,
    })
    fireEvent.click(screen.getByRole("button", { name: "Dismiss AI assistance status" }))
    await flush()
    expect(screen.queryByText("AI request failed")).toBeNull()
    expect(screen.getByText("AI · quiet")).toBeTruthy()
  })

  it("offers Re-enrich and Replace inline when the response was overtaken by an edit", async () => {
    await renderApp("local_ai")
    await editNoteText("An overtaken thought")
    await reachInFlight()
    await settle({
      status: "rejected",
      result: { noteType: "claim", labels: [], annotation: null, relatedNoteIds: [] },
      snapshot,
      reason: "the Note changed while the model was working",
    })

    fireEvent.click(screen.getByRole("button", { name: "Re-enrich and Replace" }))
    fireEvent.click(screen.getByRole("button", { name: "Replace" }))
    await flush()

    expect(enrichCalls).toEqual([
      { noteId, force: false },
      { noteId, force: true },
    ])
  })
})
