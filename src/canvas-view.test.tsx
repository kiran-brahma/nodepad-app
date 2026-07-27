import { afterEach, describe, expect, it, vi } from "vitest"
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react"
import { autoCanvasPositions, canvasRelationships, CanvasView } from "./canvas-view"
import type { Note, PendingSynthesis, Relationship } from "./workspace-client"
import { thinkingGraph } from "./thinking-graph"
import type { NoteFocus } from "./note-focus"

function note(id: string, canvasX: number | null = null, canvasY: number | null = null): Note {
  return {
    id,
    workspaceId: "w",
    markdown: id,
    noteType: "general",
    noteTypeProvenance: "default",
    annotation: null,
    annotationProvenance: "default",
    createdAt: `2026-07-25T10:0${id}:00+00:00`,
    updatedAt: "2026-07-25T10:00:00+00:00",
    pinned: false,
    canvasX,
    canvasY,
    enrichmentRevision: 0,
    lastEnrichedAt: null,
    labels: [],
  }
}

afterEach(cleanup)

function relationship(id: string, noteIdA: string, noteIdB: string): Relationship {
  return { id, workspaceId: "w", noteIdA, noteIdB, provenance: "manual", createdAt: "2026-07-26T10:00:00+00:00" }
}

function noFocus(): NoteFocus {
  return {
    focusedNoteId: null,
    litNoteIds: null,
    focusNote: vi.fn(),
    toggleFocus: vi.fn(),
    hoverNote: vi.fn(),
    registerNoteElement: vi.fn(),
  }
}

function pendingSynthesis(sourceNoteIds: string[], stale = false): PendingSynthesis {
  return {
    id: "synthesis-1",
    workspaceId: "w",
    text: "Both Notes circle the same tension.",
    sourceNoteIds,
    labels: [],
    model: "local-model",
    policy: "local_ai",
    createdAt: "2026-07-27T10:00:00+00:00",
    stale,
  }
}

type CanvasCallbacks = Partial<
  Pick<
    React.ComponentProps<typeof CanvasView>,
    "onSetPosition" | "onRelate" | "onUnrelate" | "onAcceptSynthesis" | "onDismissSynthesis"
  >
>

function canvas(notes: Note[], callbacks: CanvasCallbacks = {}, pending: PendingSynthesis[] = []) {
  return (
    <CanvasView
      notes={notes}
      graph={thinkingGraph(notes, [])}
      focus={noFocus()}
      card={() => <div />}
      suggestions={[]}
      pendingSyntheses={pending}
      onSetPosition={callbacks.onSetPosition ?? vi.fn()}
      onRelate={callbacks.onRelate ?? vi.fn()}
      onUnrelate={callbacks.onUnrelate ?? vi.fn()}
      onAcceptSynthesis={callbacks.onAcceptSynthesis ?? vi.fn()}
      onDismissSynthesis={callbacks.onDismissSynthesis ?? vi.fn()}
    />
  )
}

describe("canvas placement", () => {
  it("packs unpositioned Notes into distinct deterministic positions", () => {
    const notes = [note("1"), note("2", 30, 30), note("3")]
    expect(autoCanvasPositions(notes)).toEqual(autoCanvasPositions(notes))
    expect(new Set([...autoCanvasPositions(notes).values()].map(({ x, y }) => `${x}:${y}`)).size).toBe(2)
    expect(autoCanvasPositions(notes).get("1")).not.toEqual({ x: 28, y: 28 })
  })

  it("does not turn a card control into a drag mutation", () => {
    const commit = vi.fn()
    const { getByRole } = render(
      <CanvasView
        notes={[note("1", 10, 10)]}
        graph={thinkingGraph([note("1", 10, 10)], [])}
        focus={noFocus()}
        card={() => <button>Control</button>}
        suggestions={[]}
        pendingSyntheses={[]}
        onAcceptSynthesis={vi.fn()}
        onDismissSynthesis={vi.fn()}
        onSetPosition={commit}
        onRelate={vi.fn()}
        onUnrelate={vi.fn()}
      />,
    )
    fireEvent.pointerDown(getByRole("button", { name: "Control" }), { pointerId: 1, clientX: 20, clientY: 20 })
    expect(commit).not.toHaveBeenCalled()
  })

  it("commits every missing position exactly once through its client seam", async () => {
    const commit = vi.fn()
    render(canvas([note("1"), note("2", 500, 50)], { onSetPosition: commit }))
    await waitFor(() => expect(commit).toHaveBeenCalledTimes(1))
    expect(commit).toHaveBeenCalledWith("1", expect.any(Number), expect.any(Number))
  })

  it("relates a Note only when its handle drops on another Note", () => {
    const relate = vi.fn()
    const { container, getByRole } = render(canvas([note("1", 10, 10), note("2", 300, 10)], { onRelate: relate }))
    fireEvent.pointerDown(getByRole("button", { name: "Create Relationship from 1" }), { pointerId: 1 })
    fireEvent.pointerUp(container.querySelector('[data-note-id="2"]')!, { pointerId: 1, clientX: 310, clientY: 20 })
    expect(relate).toHaveBeenCalledWith("1", "2")
  })

  it("cancels a link dropped on empty canvas space or its own Note", () => {
    const relate = vi.fn()
    const { container, getByLabelText, getByRole } = render(canvas([note("1", 10, 10), note("2", 300, 10)], { onRelate: relate }))
    const handle = getByRole("button", { name: "Create Relationship from 1" })
    fireEvent.pointerDown(handle, { pointerId: 1 })
    fireEvent.pointerUp(getByLabelText("Note canvas"), { pointerId: 1, clientX: 700, clientY: 500 })
    fireEvent.pointerDown(handle, { pointerId: 2 })
    fireEvent.pointerUp(container.querySelector('[data-note-id="1"]')!, { pointerId: 2, clientX: 20, clientY: 20 })
    expect(relate).not.toHaveBeenCalled()
  })

  it("draws the shared Thinking Graph links and removes one from its line", () => {
    const notes = [note("1", 10, 10), note("2", 300, 10)]
    const graph = thinkingGraph(notes, [relationship("relationship-1", "1", "2")])
    const remove = vi.fn()
    const { getByRole } = render(
      <CanvasView
        notes={notes}
        graph={graph}
        focus={{ ...noFocus(), focusedNoteId: "1", litNoteIds: new Set(["1"]) }}
        card={() => <div />}
        suggestions={[]}
        pendingSyntheses={[]}
        onAcceptSynthesis={vi.fn()}
        onDismissSynthesis={vi.fn()}
        onSetPosition={vi.fn()}
        onRelate={vi.fn()}
        onUnrelate={remove}
      />,
    )
    expect(canvasRelationships(graph, new Map([["1", { x: 10, y: 10 }], ["2", { x: 300, y: 10 }]]), new Set(["1"]))).toHaveLength(1)
    const line = getByRole("button", { name: "Remove Relationship" })
    fireEvent.keyDown(line, { key: "Enter" })
    expect(remove).toHaveBeenCalledWith("1", "2")
  })
})

describe("pending Synthesis on the canvas", () => {
  const notes = [note("1", 10, 10), note("2", 300, 10)]

  it("offers a pending Synthesis among the Notes it rests on and routes both answers", () => {
    const accept = vi.fn()
    const dismiss = vi.fn()
    const { container, getByLabelText, getByRole, getByText } = render(
      canvas(notes, { onAcceptSynthesis: accept, onDismissSynthesis: dismiss }, [
        pendingSynthesis(["1", "2"]),
      ]),
    )
    const offer = getByLabelText("Pending Synthesis: Both Notes circle the same tension.")
    // Placed at the centroid of its source cards, and never a Note: it carries
    // no note id, so nothing can drag it or link a Relationship to it.
    expect([offer.style.left, offer.style.top]).toEqual(["259px", "100px"])
    expect(offer.hasAttribute("data-note-id")).toBe(false)
    expect(getByText("Synthesis forming · connects 2 Notes")).toBeTruthy()
    expect(container.querySelectorAll(".canvas-synthesis-leader")).toHaveLength(2)

    fireEvent.click(getByRole("button", { name: "Accept as thesis" }))
    fireEvent.click(getByRole("button", { name: "Dismiss" }))
    expect(accept).toHaveBeenCalledWith("synthesis-1")
    expect(dismiss).toHaveBeenCalledWith("synthesis-1")
  })

  it("shows a stale Synthesis dimmed and lets it only be dismissed", () => {
    const dismiss = vi.fn()
    const { getByLabelText, getByRole } = render(
      canvas(notes, { onDismissSynthesis: dismiss }, [pendingSynthesis(["1", "2"], true)]),
    )
    expect(getByLabelText("Pending Synthesis: Both Notes circle the same tension.").className)
      .toContain("stale")
    expect((getByRole("button", { name: "Accept as thesis" }) as HTMLButtonElement).disabled).toBe(true)
    fireEvent.click(getByRole("button", { name: "Dismiss" }))
    expect(dismiss).toHaveBeenCalledWith("synthesis-1")
  })

  it("offers nothing when none is pending or none of its sources is drawn", () => {
    const { container: empty } = render(canvas(notes))
    expect(empty.querySelector(".canvas-synthesis")).toBeNull()
    cleanup()
    const { container: undrawn } = render(canvas(notes, {}, [pendingSynthesis(["gone"])]))
    expect(undrawn.querySelector(".canvas-synthesis")).toBeNull()
  })
})
