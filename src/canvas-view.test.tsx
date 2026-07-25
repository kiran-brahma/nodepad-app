import { afterEach, describe, expect, it, vi } from "vitest"
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react"
import { autoCanvasPositions, CanvasView } from "./canvas-view"
import type { Note } from "./workspace-client"

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
      <CanvasView notes={[note("1", 10, 10)]} card={() => <button>Control</button>} onSetPosition={commit} />,
    )
    fireEvent.pointerDown(getByRole("button", { name: "Control" }), { pointerId: 1, clientX: 20, clientY: 20 })
    expect(commit).not.toHaveBeenCalled()
  })

  it("commits every missing position exactly once through its client seam", async () => {
    const commit = vi.fn()
    render(<CanvasView notes={[note("1"), note("2", 500, 50)]} card={() => <div />} onSetPosition={commit} />)
    await waitFor(() => expect(commit).toHaveBeenCalledTimes(1))
    expect(commit).toHaveBeenCalledWith("1", expect.any(Number), expect.any(Number))
  })
})
