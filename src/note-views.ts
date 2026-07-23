import { NOTE_TYPES, type Note, type NoteType, type SearchResult } from "./workspace-client"

/**
 * How the same committed Notes are arranged on screen. A view is a way of
 * reading a Thinking Workspace, never a place a Note lives, so nothing here is
 * committed and no arrangement changes what a Note means.
 */
export const NOTE_VIEWS = ["tiling", "kanban", "graph"] as const

export type NoteView = (typeof NOTE_VIEWS)[number]

const NOTE_VIEW_LABELS: Record<NoteView, string> = {
  tiling: "Tiling",
  kanban: "Kanban",
  graph: "Graph",
}

export function noteViewLabel(view: NoteView): string {
  return NOTE_VIEW_LABELS[view]
}

/**
 * A search narrows what both views show, so a result contributes only the
 * identity of the Note it found. Note content is always read from the
 * committed Note, never from a snippet, so a search can never become a second
 * copy of the thinking.
 */
export function matchingNoteIds(results: SearchResult[] | null): ReadonlySet<string> | null {
  return results ? new Set(results.map((result) => result.noteId)) : null
}

/** Every Note of one Thinking Workspace, whatever any search is narrowing to. */
export function workspaceNotes(notes: Note[], workspaceId: string | undefined): Note[] {
  if (!workspaceId) return []
  return notes.filter((note) => note.workspaceId === workspaceId)
}

/**
 * The one result set both views read: the Notes of the active Thinking
 * Workspace, narrowed by any active search, pinned Notes first, and stable
 * creation order breaking ties.
 */
export function visibleNotes(
  notes: Note[],
  workspaceId: string | undefined,
  matching: ReadonlySet<string> | null,
): Note[] {
  return workspaceNotes(notes, workspaceId)
    .filter((note) => !matching || matching.has(note.id))
    .sort(
      (left, right) =>
        Number(right.pinned) - Number(left.pinned) ||
        left.createdAt.localeCompare(right.createdAt) ||
        left.id.localeCompare(right.id),
    )
}

/** How many Notes share one tiled page before the next page begins. */
export const TILING_PAGE_SIZE = 7

/** Pages are slices of the one result set, so paging can reorder nothing. */
export function tilingPages(notes: Note[]): Note[][] {
  const pages: Note[][] = []
  for (let start = 0; start < notes.length; start += TILING_PAGE_SIZE) {
    pages.push(notes.slice(start, start + TILING_PAGE_SIZE))
  }
  return pages
}

/**
 * A page arranged by repeated halving, alternating direction with depth. It is
 * derived from the page's order alone: no coordinate is stored, so a restart
 * reconstructs the same arrangement from SQLite.
 */
export type NoteArrangement =
  | { kind: "note"; note: Note }
  | {
      kind: "split"
      direction: "row" | "column"
      first: NoteArrangement
      second: NoteArrangement
    }

function halve(page: Note[], depth: number): NoteArrangement {
  if (page.length === 1) return { kind: "note", note: page[0] }
  const half = Math.floor(page.length / 2)
  return {
    kind: "split",
    direction: depth % 2 === 0 ? "row" : "column",
    first: halve(page.slice(0, half), depth + 1),
    second: halve(page.slice(half), depth + 1),
  }
}

/** A page with no Note has no arrangement; every other page has exactly one. */
export function noteArrangement(page: Note[]): NoteArrangement | null {
  return page.length === 0 ? null : halve(page, 0)
}

/** A side's share of its split: how many Notes it shows. */
export function arrangementWeight(arrangement: NoteArrangement): number {
  if (arrangement.kind === "note") return 1
  return arrangementWeight(arrangement.first) + arrangementWeight(arrangement.second)
}

export interface KanbanColumn {
  noteType: NoteType
  notes: Note[]
}

/**
 * One column per Note Type present in the result, in the fixed order the
 * durable interface declares its Note Types. A Note Type nobody used has no
 * column, so a filtered result can never show a stale one.
 */
export function kanbanColumns(notes: Note[]): KanbanColumn[] {
  return NOTE_TYPES.map((noteType) => ({
    noteType,
    notes: notes.filter((note) => note.noteType === noteType),
  })).filter((column) => column.notes.length > 0)
}

/**
 * Selection is transient and belongs to the reader, so switching view keeps it
 * while the Note is still on screen and otherwise lets it go.
 */
export function preservedSelection(selectedNoteId: string | null, visible: Note[]): string | null {
  if (!selectedNoteId) return null
  return visible.some((note) => note.id === selectedNoteId) ? selectedNoteId : null
}
