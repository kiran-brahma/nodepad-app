import { describe, expect, it } from "vitest"
import {
  litNoteIds,
  nodeDegree,
  relatableNotes,
  relatedNoteIds,
  relatedNotes,
  thinkingGraph,
} from "./thinking-graph"
import type { Note, Relationship } from "./workspace-client"

function note(id: string, markdown: string, workspaceId = "workspace-1"): Note {
  return {
    id,
    workspaceId,
    markdown,
    noteType: "general",
    noteTypeProvenance: "default",
    annotation: null,
    annotationProvenance: "default",
    createdAt: "2026-07-22T10:00:00+00:00",
    updatedAt: "2026-07-22T10:00:00+00:00",
    pinned: false,
    labels: [],
  }
}

/** Endpoints arrive canonically ordered, as the durable interface stores them. */
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

const rivers = note("note-1", "Cities grew around rivers")
const trade = note("note-2", "Trade follows water")
const drought = note("note-3", "Drought moves people")
const elsewhere = note("note-4", "Another Workspace entirely", "workspace-2")
/** One Thinking Workspace's Notes, which is what the projection is given. */
const notes = [rivers, trade, drought]

function ids(result: Note[]): string[] {
  return result.map(({ id }) => id)
}

describe("the Thinking Graph projection", () => {
  it("gives every Note one node and every Relationship one undirected link", () => {
    const graph = thinkingGraph(notes, [
      relationship("relationship-1", trade.id, rivers.id),
      relationship("relationship-2", rivers.id, drought.id),
    ])
    expect(graph.nodes.map((node) => node.note.id)).toEqual([rivers.id, trade.id, drought.id])
    expect(graph.links).toEqual([
      { id: "relationship-1", noteIdA: rivers.id, noteIdB: trade.id },
      { id: "relationship-2", noteIdA: rivers.id, noteIdB: drought.id },
    ])
  })

  it("reads a link from either endpoint, because a pair has no direction", () => {
    const graph = thinkingGraph(notes, [relationship("relationship-1", trade.id, rivers.id)])
    expect(relatedNoteIds(graph, rivers.id)).toEqual([trade.id])
    expect(relatedNoteIds(graph, trade.id)).toEqual([rivers.id])
    expect(relatedNoteIds(graph, drought.id)).toEqual([])
  })

  it("counts a node's degree as the links drawn to it, for every Note", () => {
    const graph = thinkingGraph(notes, [
      relationship("relationship-1", rivers.id, trade.id),
      relationship("relationship-2", rivers.id, drought.id),
    ])
    for (const node of graph.nodes) {
      expect(node.degree).toBe(relatedNoteIds(graph, node.note.id).length)
      expect(node.degree).toBe(relatedNotes(graph, node.note.id).length)
      expect(nodeDegree(graph, node.note.id)).toBe(node.degree)
    }
    expect(graph.nodes.map((node) => node.degree)).toEqual([2, 1, 1])
    expect(nodeDegree(graph, "no such Note")).toBe(0)
  })

  it("draws no link to a Note that is not in this Thinking Workspace", () => {
    // A Relationship left behind by a moved Note has no node to attach to.
    const graph = thinkingGraph(notes, [
      relationship("relationship-1", rivers.id, elsewhere.id),
      relationship("relationship-2", "deleted-note", trade.id),
    ])
    expect(graph.links).toEqual([])
    expect(graph.nodes.every((node) => node.degree === 0)).toBe(true)
  })

  it("admits one canonical pair once, however many times it is offered", () => {
    const graph = thinkingGraph(notes, [
      relationship("relationship-1", rivers.id, trade.id),
      relationship("relationship-2", trade.id, rivers.id),
    ])
    expect(graph.links.map((link) => link.id)).toEqual(["relationship-1"])
    expect(nodeDegree(graph, rivers.id)).toBe(1)
  })

  it("draws no link from a Note to itself", () => {
    const graph = thinkingGraph(notes, [relationship("relationship-1", rivers.id, rivers.id)])
    expect(graph.links).toEqual([])
  })

  it("projects an empty Thinking Workspace as an empty graph", () => {
    const graph = thinkingGraph([], [relationship("relationship-1", rivers.id, trade.id)])
    expect(graph).toEqual({ nodes: [], links: [] })
    expect(litNoteIds(graph, null)).toBeNull()
  })

  it("resolves related endpoints to durable Notes and shows no other Note", () => {
    const graph = thinkingGraph(notes, [relationship("relationship-1", rivers.id, drought.id)])
    expect(relatedNotes(graph, rivers.id)).toEqual([drought])
    expect(relatedNotes(graph, trade.id)).toEqual([])
  })
})

describe("what a focus lights", () => {
  const graph = thinkingGraph(notes, [relationship("relationship-1", rivers.id, trade.id)])

  it("lights the focal Note and exactly the Notes related to it", () => {
    expect(litNoteIds(graph, rivers.id)).toEqual(new Set([rivers.id, trade.id]))
    expect(litNoteIds(graph, drought.id)).toEqual(new Set([drought.id]))
  })

  it("lights nothing while nothing is focused, so nothing is dimmed", () => {
    expect(litNoteIds(graph, null)).toBeNull()
  })

  it("agrees with degree for every Note", () => {
    for (const node of graph.nodes) {
      expect(litNoteIds(graph, node.note.id)!.size).toBe(node.degree + 1)
    }
  })
})

describe("the Notes a Relationship may be created to", () => {
  it("offers no Note twice, itself, or one from another Thinking Workspace", () => {
    const related = thinkingGraph(notes, [relationship("relationship-1", rivers.id, trade.id)])
    expect(ids(relatableNotes(related, rivers.id, ""))).toEqual([drought.id])

    // With nothing related yet, every other Note in the Workspace is offered.
    const unrelated = thinkingGraph(notes, [])
    expect(ids(relatableNotes(unrelated, rivers.id, ""))).toEqual([trade.id, drought.id])

    const mixed = thinkingGraph([...notes, elsewhere], [])
    expect(ids(relatableNotes(mixed, elsewhere.id, ""))).toEqual([])
    expect(ids(relatableNotes(mixed, rivers.id, ""))).toEqual([trade.id, drought.id])
  })

  it("narrows candidates by the thinker's text across Note and Annotation", () => {
    const annotated = { ...drought, annotation: "Migration pressure" }
    const graph = thinkingGraph([rivers, trade, annotated], [])
    expect(relatableNotes(graph, rivers.id, "water")).toEqual([trade])
    expect(relatableNotes(graph, rivers.id, "  MIGRATION  ")).toEqual([annotated])
    expect(relatableNotes(graph, rivers.id, "nothing here")).toEqual([])
  })

  it("offers nothing for a Note that no longer exists", () => {
    expect(relatableNotes(thinkingGraph(notes, []), "vanished", "")).toEqual([])
  })
})
