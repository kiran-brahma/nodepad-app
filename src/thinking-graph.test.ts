import { describe, expect, it } from "vitest"
import { degree, relatableNotes, relatedNoteIds, relatedNotes } from "./thinking-graph"
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
const notes = [rivers, trade, drought, elsewhere]

describe("the Thinking Graph projections", () => {
  it("reads a Relationship from either endpoint", () => {
    const relationships = [relationship("relationship-1", trade.id, rivers.id)]
    expect(relatedNoteIds(relationships, rivers.id)).toEqual([trade.id])
    expect(relatedNoteIds(relationships, trade.id)).toEqual([rivers.id])
    expect(relatedNoteIds(relationships, drought.id)).toEqual([])
  })

  it("counts each Relationship once for each of its endpoints", () => {
    const relationships = [
      relationship("relationship-1", rivers.id, trade.id),
      relationship("relationship-2", rivers.id, drought.id),
    ]
    expect(degree(relationships, rivers.id)).toBe(2)
    expect(degree(relationships, trade.id)).toBe(1)
    expect(degree(relationships, drought.id)).toBe(1)
    expect(degree(relationships, elsewhere.id)).toBe(0)
  })

  it("resolves related endpoints to durable Notes and shows no other Note", () => {
    const relationships = [relationship("relationship-1", rivers.id, drought.id)]
    expect(relatedNotes(notes, relationships, rivers.id)).toEqual([drought])
    expect(relatedNotes(notes, relationships, trade.id)).toEqual([])
  })

  it("offers no Note twice, itself, or one from another Thinking Workspace", () => {
    const relationships = [relationship("relationship-1", rivers.id, trade.id)]
    expect(relatableNotes(notes, relationships, rivers.id, "")).toEqual([drought])
    // With nothing related yet, every other Note in the Workspace is offered.
    expect(relatableNotes(notes, [], rivers.id, "")).toEqual([trade, drought])
    expect(relatableNotes(notes, [], elsewhere.id, "")).toEqual([])
  })

  it("narrows candidates by the thinker's text across Note and Annotation", () => {
    const annotated = { ...drought, annotation: "Migration pressure" }
    const searchable = [rivers, trade, annotated]
    expect(relatableNotes(searchable, [], rivers.id, "water")).toEqual([trade])
    expect(relatableNotes(searchable, [], rivers.id, "  MIGRATION  ")).toEqual([annotated])
    expect(relatableNotes(searchable, [], rivers.id, "nothing here")).toEqual([])
  })

  it("offers nothing for a Note that no longer exists", () => {
    expect(relatableNotes(notes, [], "vanished", "")).toEqual([])
  })
})
