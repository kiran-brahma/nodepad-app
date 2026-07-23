import { describe, expect, it } from "vitest"
import type { Note, Relationship } from "./workspace-client"
import { thinkingGraph } from "./thinking-graph"
import {
  graphLayout,
  GRAPH_HEIGHT,
  GRAPH_WIDTH,
  MIN_NODE_RADIUS,
  nodeRadius,
} from "./graph-layout"

function note(id: string): Note {
  return {
    id,
    workspaceId: "workspace-1",
    markdown: id,
    noteType: "general",
    noteTypeProvenance: "default",
    annotation: null,
    annotationProvenance: "default",
    createdAt: "2026-07-22T10:00:00+00:00",
    updatedAt: "2026-07-22T10:00:00+00:00",
    pinned: false,
    labels: [],
    enrichmentRevision: 0,
    lastEnrichedAt: null,
  }
}

function relationship(id: string, left: string, right: string): Relationship {
  const [noteIdA, noteIdB] = left <= right ? [left, right] : [right, left]
  return {
    id,
    workspaceId: "workspace-1",
    noteIdA,
    noteIdB,
    provenance: "manual",
    createdAt: "2026-07-22T11:00:00+00:00",
  }
}

const notes = Array.from({ length: 6 }, (_, index) => note(`note-${index}`))

/** A hub, a pair, and one Note related to nothing at all. */
const relationships = [
  relationship("relationship-1", "note-0", "note-1"),
  relationship("relationship-2", "note-0", "note-2"),
  relationship("relationship-3", "note-0", "note-3"),
  relationship("relationship-4", "note-4", "note-1"),
]

describe("how large a node is drawn", () => {
  it("keeps an unrelated Note a visible node", () => {
    expect(nodeRadius(0, 0)).toBe(MIN_NODE_RADIUS)
    expect(nodeRadius(0, 4)).toBe(MIN_NODE_RADIUS)
  })

  it("grows with degree, never shrinking as a Note gains Relationships", () => {
    const radii = [0, 1, 2, 3, 4].map((degree) => nodeRadius(degree, 4))
    for (let index = 1; index < radii.length; index += 1) {
      expect(radii[index]).toBeGreaterThan(radii[index - 1])
    }
  })
})

describe("the arrangement the graph is drawn in", () => {
  const graph = thinkingGraph(notes, relationships)
  const layout = graphLayout(graph)

  it("places every Note exactly once", () => {
    expect(layout.placements.map((placement) => placement.node.note.id)).toEqual(
      notes.map(({ id }) => id),
    )
  })

  it("places every Note whole inside the canvas, isolated Notes included", () => {
    for (const placement of layout.placements) {
      expect(placement.x).toBeGreaterThanOrEqual(placement.radius)
      expect(placement.x).toBeLessThanOrEqual(GRAPH_WIDTH - placement.radius)
      expect(placement.y).toBeGreaterThanOrEqual(placement.radius)
      expect(placement.y).toBeLessThanOrEqual(GRAPH_HEIGHT - placement.radius)
      expect(placement.radius).toBeGreaterThanOrEqual(MIN_NODE_RADIUS)
    }
  })

  it("draws one line per Relationship, between two placed Notes", () => {
    expect(layout.links.map((link) => link.id)).toEqual(graph.links.map((link) => link.id))
    for (const link of layout.links) {
      expect(layout.placements).toContain(link.source)
      expect(layout.placements).toContain(link.target)
      expect(link.source).not.toBe(link.target)
    }
  })

  it("draws the most related Note larger than one related to nothing", () => {
    const hub = layout.placements.find((placement) => placement.node.note.id === "note-0")!
    const isolated = layout.placements.find((placement) => placement.node.note.id === "note-5")!
    expect(hub.node.degree).toBe(3)
    expect(isolated.node.degree).toBe(0)
    expect(hub.radius).toBeGreaterThan(isolated.radius)
  })

  it("rebuilds the same arrangement from the same committed state", () => {
    // Nothing is carried between calls, so a restart draws the same picture.
    expect(graphLayout(thinkingGraph(notes, relationships))).toEqual(layout)
  })

  it("arranges an empty, a single-Note, and a disconnected Workspace safely", () => {
    expect(graphLayout(thinkingGraph([], []))).toEqual({ placements: [], links: [] })

    const alone = graphLayout(thinkingGraph([notes[0]], relationships))
    expect(alone.placements).toHaveLength(1)
    expect(alone.links).toEqual([])
    expect(alone.placements[0].radius).toBe(MIN_NODE_RADIUS)

    const disconnected = graphLayout(thinkingGraph(notes, []))
    expect(disconnected.placements).toHaveLength(notes.length)
    expect(disconnected.links).toEqual([])
  })

  it("keeps a dense graph inside the canvas", () => {
    const many = Array.from({ length: 30 }, (_, index) => note(`dense-${index}`))
    const dense = many.flatMap((source, index) =>
      many
        .slice(index + 1)
        .map((target) => relationship(`dense-${source.id}-${target.id}`, source.id, target.id)),
    )
    const layout = graphLayout(thinkingGraph(many, dense))
    expect(layout.placements).toHaveLength(many.length)
    expect(layout.links).toHaveLength(dense.length)
    for (const placement of layout.placements) {
      expect(placement.x).toBeGreaterThanOrEqual(0)
      expect(placement.x).toBeLessThanOrEqual(GRAPH_WIDTH)
      expect(placement.y).toBeGreaterThanOrEqual(0)
      expect(placement.y).toBeLessThanOrEqual(GRAPH_HEIGHT)
    }
  })
})
