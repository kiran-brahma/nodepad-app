import { describe, expect, it } from "vitest"
import type { Note, NoteType, SearchResult } from "./workspace-client"
import {
  kanbanColumns,
  matchingNoteIds,
  preservedSelection,
  noteArrangement,
  arrangementWeight,
  tilingPages,
  TILING_PAGE_SIZE,
  visibleNotes,
} from "./note-views"

function note(
  id: string,
  fields: Partial<Note> & { createdAt: string; workspaceId?: string },
): Note {
  return {
    id,
    workspaceId: "workspace-1",
    markdown: id,
    noteType: "general",
    noteTypeProvenance: "default",
    annotation: null,
    annotationProvenance: "default",
    updatedAt: fields.createdAt,
    pinned: false,
    labels: [],
    enrichmentRevision: 0,
    lastEnrichedAt: null,
    ...fields,
  }
}

/** A committed Workspace snapshot, given out of order on purpose. */
const notes: Note[] = [
  note("note-3", { createdAt: "2026-07-22T10:03:00+00:00", noteType: "question" }),
  note("note-1", { createdAt: "2026-07-22T10:01:00+00:00", noteType: "claim" }),
  note("note-4", { createdAt: "2026-07-22T10:04:00+00:00", pinned: true }),
  note("note-2", { createdAt: "2026-07-22T10:02:00+00:00", noteType: "claim" }),
  note("elsewhere", { createdAt: "2026-07-22T10:00:00+00:00", workspaceId: "workspace-2" }),
]

function ids(result: Note[]): string[] {
  return result.map(({ id }) => id)
}

function results(...noteIds: string[]): SearchResult[] {
  return noteIds.map((noteId) => ({
    noteId,
    snippet: "a snippet no view reads",
    noteType: "general" as NoteType,
    labels: [],
    enrichmentRevision: 0,
    lastEnrichedAt: null,
    rank: 0,
  }))
}

describe("the result set both views read", () => {
  it("shows only the active Thinking Workspace, pinned first, then creation order", () => {
    expect(ids(visibleNotes(notes, "workspace-1", null))).toEqual([
      "note-4",
      "note-1",
      "note-2",
      "note-3",
    ])
  })

  it("shows nothing while no Thinking Workspace is active", () => {
    expect(visibleNotes(notes, undefined, null)).toEqual([])
  })

  it("narrows to the Notes a search found, keeping the same order", () => {
    const matching = matchingNoteIds(results("note-3", "note-4"))
    expect(ids(visibleNotes(notes, "workspace-1", matching))).toEqual(["note-4", "note-3"])
  })

  it("shows every Note when no search is active", () => {
    expect(matchingNoteIds(null)).toBeNull()
  })

  it("shows no Note when a search found none", () => {
    expect(visibleNotes(notes, "workspace-1", matchingNoteIds([]))).toEqual([])
  })
})

describe("tiling pages", () => {
  const many = Array.from({ length: TILING_PAGE_SIZE + 2 }, (_, index) =>
    note(`note-${index}`, { createdAt: `2026-07-22T10:0${index}:00+00:00` }),
  )

  it("gives an empty result no page at all", () => {
    expect(tilingPages([])).toEqual([])
    expect(noteArrangement([])).toBeNull()
  })

  it("fills a page before starting the next one, in the order it was given", () => {
    const pages = tilingPages(many)
    expect(pages.map((page) => page.length)).toEqual([TILING_PAGE_SIZE, 2])
    expect(pages.flatMap(ids)).toEqual(ids(many))
  })

  it("arranges one Note as the whole page", () => {
    const arrangement = noteArrangement(many.slice(0, 1))!
    expect(arrangement).toEqual({ kind: "note", note: many[0] })
    expect(arrangementWeight(arrangement)).toBe(1)
  })

  it("halves a page, alternating direction with depth", () => {
    const arrangement = noteArrangement(many.slice(0, 4))!
    expect(arrangement.kind).toBe("split")
    if (arrangement.kind !== "split") return
    expect(arrangement.direction).toBe("row")
    expect(arrangement.first.kind === "split" && arrangement.first.direction).toBe("column")
    expect(arrangementWeight(arrangement)).toBe(4)
  })

  it("gives every Note on the page exactly one place", () => {
    const page = many.slice(0, TILING_PAGE_SIZE)
    const arranged: string[] = []
    const walk = (arrangement: ReturnType<typeof noteArrangement>) => {
      if (!arrangement) return
      if (arrangement.kind === "note") arranged.push(arrangement.note.id)
      else {
        walk(arrangement.first)
        walk(arrangement.second)
      }
    }
    walk(noteArrangement(page))
    expect(arranged).toEqual(ids(page))
  })
})

describe("kanban columns", () => {
  it("gives one column per Note Type present, in the fixed Note Type order", () => {
    const visible = visibleNotes(notes, "workspace-1", null)
    expect(kanbanColumns(visible).map((column) => column.noteType)).toEqual([
      "claim",
      "question",
      "general",
    ])
  })

  it("keeps the result set's order inside a column", () => {
    const visible = visibleNotes(notes, "workspace-1", null)
    const claims = kanbanColumns(visible).find((column) => column.noteType === "claim")!
    expect(ids(claims.notes)).toEqual(["note-1", "note-2"])
  })

  it("shows the same Notes as tiling, and no empty column", () => {
    const visible = visibleNotes(notes, "workspace-1", matchingNoteIds(results("note-3")))
    expect(kanbanColumns(visible).map((column) => column.noteType)).toEqual(["question"])
    expect(kanbanColumns(visible).flatMap((column) => ids(column.notes))).toEqual(
      tilingPages(visible).flatMap(ids),
    )
  })

  it("gives an empty result no column", () => {
    expect(kanbanColumns([])).toEqual([])
  })
})

describe("selection across a view switch", () => {
  const visible = visibleNotes(notes, "workspace-1", null)

  it("keeps the selected Note while it is still on screen", () => {
    expect(preservedSelection("note-2", visible)).toBe("note-2")
  })

  it("lets the selection go when the Note is no longer on screen", () => {
    expect(preservedSelection("elsewhere", visible)).toBeNull()
  })

  it("selects nothing when nothing was selected", () => {
    expect(preservedSelection(null, visible)).toBeNull()
  })
})
