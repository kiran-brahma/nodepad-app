import { afterEach, describe, expect, it, vi } from "vitest"
import { cleanup, render, screen } from "@testing-library/react"
import { GraphView } from "./graph-view"
import { thinkingGraph } from "./thinking-graph"
import type { NoteFocus } from "./note-focus"
import type { Note, PendingSynthesis } from "./workspace-client"

function note(id: string, markdown: string): Note {
  return {
    id,
    workspaceId: "w",
    markdown,
    noteType: "claim",
    noteTypeProvenance: "manual",
    annotation: null,
    annotationProvenance: "default",
    createdAt: "2026-07-22T10:00:00+00:00",
    updatedAt: "2026-07-22T10:00:00+00:00",
    pinned: false,
    enrichmentRevision: 0,
    lastEnrichedAt: null,
    labels: [],
  }
}

const notes = [note("n1", "Cities grew around rivers"), note("n2", "Trade follows water")]

const focus: NoteFocus = {
  focusedNoteId: null,
  litNoteIds: null,
  focusNote: vi.fn(),
  toggleFocus: vi.fn(),
  hoverNote: vi.fn(),
  registerNoteElement: vi.fn(),
}

const pending: PendingSynthesis = {
  id: "s1",
  workspaceId: "w",
  text: "Water moved goods before it moved people, so the trade routes chose the cities.",
  sourceNoteIds: ["n1", "n2"],
  labels: [],
  model: "phi3:latest",
  policy: "local_ai",
  createdAt: "2026-07-23T10:00:00+00:00",
  stale: false,
}

function draw(pendingSyntheses: PendingSynthesis[]) {
  // No Relationship exists between these Notes; every line the graph draws
  // for a pending Synthesis is therefore provisional by construction.
  const graph = thinkingGraph(notes, [])
  return render(
    <GraphView
      graph={graph}
      focus={focus}
      card={() => null}
      pendingSyntheses={pendingSyntheses}
    />,
  )
}

afterEach(cleanup)

describe("a pending Synthesis in the graph", () => {
  it("is drawn distinctly, without a Relationship or a Note node", () => {
    draw([pending])
    const canvas = screen.getByRole("group", { name: "Thinking Graph" })
    // One Note node per Note, and no more: a Synthesis is not a Note.
    expect(screen.getAllByRole("button")).toHaveLength(notes.length)
    const mark = screen.getByRole("img", { name: /Pending Synthesis/ })
    expect(mark.getAttribute("class")).toContain("graph-synthesis-mark")
    // Its leaders are dashed, so they never read as committed Relationships.
    const leaders = canvas.querySelectorAll("line.graph-synthesis-leader")
    expect(leaders).toHaveLength(pending.sourceNoteIds.length)
    for (const leader of leaders) {
      expect(leader.getAttribute("stroke-dasharray")).toBeTruthy()
    }
    // And the committed Thinking Graph is still empty.
    expect(canvas.querySelectorAll("line.graph-link")).toHaveLength(0)
  })

  it("disappears the moment it is no longer pending", () => {
    draw([])
    expect(screen.queryByRole("img", { name: /Pending Synthesis/ })).toBeNull()
    expect(
      screen
        .getByRole("group", { name: "Thinking Graph" })
        .querySelectorAll("line.graph-synthesis-leader"),
    ).toHaveLength(0)
  })

  it("is not drawn when the Notes it names have left the canvas", () => {
    draw([{ ...pending, sourceNoteIds: ["gone", "also-gone"] }])
    expect(screen.queryByRole("img", { name: /Pending Synthesis/ })).toBeNull()
  })
})
