import type { ReactNode } from "react"
import type { Note, PendingSynthesis } from "./workspace-client"
import { noteViewLabel, NOTE_VIEWS, type NoteView } from "./note-views"
import { TilingView } from "./tiling-view"
import { KanbanView } from "./kanban-view"
import { GraphView } from "./graph-view"
import type { ThinkingGraph } from "./thinking-graph"
import type { NoteFocus } from "./note-focus"

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
  onChooseView,
  onUndo,
  card,
  pendingSyntheses,
}: {
  notes: Note[]
  /** The whole Thinking Graph of the active Workspace, which no search narrows. */
  graph: ThinkingGraph
  focus: NoteFocus
  searching: boolean
  view: NoteView
  canUndo: boolean
  onChooseView: (view: NoteView) => void
  onUndo: () => void
  card: (note: Note) => ReactNode
  /** Undecided Syntheses, drawn provisionally by the graph and by nothing
   *  else. They are not Notes, so no other view arranges them. */
  pendingSyntheses: PendingSynthesis[]
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

      {/* A view is a way of reading the same committed Notes. Choosing one
          commits nothing and changes no Note. */}
      <div className="row" role="group" aria-label="Note view">
        {NOTE_VIEWS.map((option) => (
          <button
            key={option}
            aria-pressed={view === option}
            className={view === option ? "active" : ""}
            onClick={() => onChooseView(option)}
          >
            {noteViewLabel(option)}
          </button>
        ))}
      </div>
      {/* The graph shows the Thinking Graph of the whole active Workspace, so
          it reads the projection rather than the searched-narrowed result. */}
      {view === "graph" ? (
        <GraphView graph={graph} focus={focus} card={card} pendingSyntheses={pendingSyntheses} />
      ) : notes.length === 0 ? (
        <p>{searching ? "No Notes match this search." : "No Notes yet."}</p>
      ) : view === "tiling" ? (
        <TilingView notes={notes} card={card} />
      ) : (
        <KanbanView notes={notes} card={card} />
      )}
    </section>
  )
}
