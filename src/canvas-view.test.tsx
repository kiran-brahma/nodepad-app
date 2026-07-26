import { afterEach, describe, expect, it, vi } from "vitest"
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react"
import { autoCanvasPositions, canvasRelationships, CanvasView } from "./canvas-view"
import type { Note, Relationship } from "./workspace-client"
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

function canvas(
  notes: Note[],
  callbacks: Partial<Pick<React.ComponentProps<typeof CanvasView>, "onSetPosition" | "onRelate" | "onUnrelate">> = {},
) {
  return (
    <CanvasView
      notes={notes}
      graph={thinkingGraph(notes, [])}
      focus={noFocus()}
      card={() => <div />}
      onSetPosition={callbacks.onSetPosition ?? vi.fn()}
      onRelate={callbacks.onRelate ?? vi.fn()}
      onUnrelate={callbacks.onUnrelate ?? vi.fn()}
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

  it("renders an uncommitted suggested Relationship as a dashed canvas line", () => {
    const notes = [note("1", 10, 10), note("2", 300, 10)]
    const { getByLabelText } = render(
      <CanvasView
        notes={notes}
        graph={thinkingGraph(notes, [])}
        focus={noFocus()}
        card={() => <div />}
        onSetPosition={vi.fn()}
        onRelate={vi.fn()}
        onUnrelate={vi.fn()}
        suggestedRelationships={[{ noteId: "1", otherNoteId: "2" }]}
      />,
    )
    expect(getByLabelText("Suggested Relationship").getAttribute("class")).toContain("suggested")
  })
})
