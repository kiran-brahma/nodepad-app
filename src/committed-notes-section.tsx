import type { ReactNode } from "react"
import type { Note, NoteType, PendingSynthesis } from "./workspace-client"
import { type NoteView } from "./note-views"
import { KanbanView } from "./kanban-view"
import { GraphView } from "./graph-view"
import { CanvasView } from "./canvas-view"
import type { ThinkingGraph } from "./thinking-graph"
import type { NoteFocus } from "./note-focus"
import type { SuggestedRelationship } from "./suggested-relationships"

/**
 * The committed Notes, arranged however the thinker chose to read them. The
 * section picks a layout and hands it the one card; it never decides what may
 * be done to a Note.
 */
export function CommittedNotesSection({
  notes,
  graph,
  focus,
  searching,
  view,
  canUndo,
  onUndo,
  onSetPosition,
  onSetNoteType,
  onRelate,
  onUnrelate,
  card,
  pendingSyntheses,
  suggestions,
}: {
  notes: Note[]
  /** The whole Thinking Graph of the active Workspace, which no search narrows. */
  graph: ThinkingGraph
  focus: NoteFocus
  searching: boolean
  view: NoteView
  canUndo: boolean
  onUndo: () => void
  onSetPosition: (noteId: string, x: number, y: number) => void
  onSetNoteType: (note: Note, noteType: NoteType) => void
  onRelate: (noteId: string, otherNoteId: string) => void
  onUnrelate: (noteId: string, otherNoteId: string) => void
  card: (note: Note) => ReactNode
  /** Undecided Syntheses, drawn provisionally by the graph and by nothing
   *  else. They are not Notes, so no other view arranges them. */
  pendingSyntheses: PendingSynthesis[]
  /** Undecided AI-proposed Relationships, drawn dashed by the canvas. They
   *  are not Relationships, so the graph never carries them. */
  suggestions: readonly SuggestedRelationship[]
}) {
  return (
    <section aria-label="Committed Notes">
      <div className="row">
        <h2>Committed Notes</h2>
        <button
          onClick={onUndo}
          disabled={!canUndo}
          title="Undo the last change in this Thinking Workspace (⌘Z)"
        >
          Undo
        </button>
      </div>
      {/* The graph shows the Thinking Graph of the whole active Workspace, so
          it reads the projection rather than the searched-narrowed result. */}
      {view === "graph" ? (
        <GraphView graph={graph} focus={focus} card={card} pendingSyntheses={pendingSyntheses} />
      ) : notes.length === 0 ? (
        searching ? (
          <p>No Notes match this search.</p>
        ) : (
          <div className="empty-workspace">
            <h2>Capture your first thought</h2>
            <p>Type a thought below and press Enter. Nodepad commits it locally before it appears.</p>
          </div>
        )
      ) : view === "canvas" ? (
        <CanvasView
          // A canvas is the spatial Thinking Graph, not a filtered result
          // list: every committed Relationship must keep both card endpoints.
          notes={graph.nodes.map((node) => node.note)}
          graph={graph}
          focus={focus}
          card={card}
          suggestions={suggestions}
          onSetPosition={onSetPosition}
          onRelate={onRelate}
          onUnrelate={onUnrelate}
        />
      ) : (
        <KanbanView notes={notes} focus={focus} card={card} onSetNoteType={onSetNoteType} />
      )}
    </section>
  )
}
