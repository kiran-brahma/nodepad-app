import type { ReactNode } from "react"
import type { Note } from "./workspace-client"
import { noteTypeLabel } from "./note-controls"
import { kanbanColumns } from "./note-views"
import type { NoteFocus } from "./note-focus"

/**
 * The visible Notes in one column per Note Type present. The column decides
 * where a Note appears and nothing else: it places the same card the tiling
 * view places, over the same intents.
 */
export function KanbanView({ notes, focus, card }: { notes: Note[]; focus: NoteFocus; card: (note: Note) => ReactNode }) {
  return (
    <div className="kanban">
      {kanbanColumns(notes).map((column) => (
        <div className="kanban-column" key={column.noteType}>
          <div className="row">
            <h3>{noteTypeLabel(column.noteType)}</h3>
            <span className="badge">{column.notes.length}</span>
          </div>
          <div role="group" aria-label={`${noteTypeLabel(column.noteType)} Notes`}>
            {column.notes.map((note) => (
              <div
                key={note.id}
                onMouseEnter={() => focus.hoverNote(note.id)}
                onMouseLeave={() => focus.hoverNote(null)}
                onFocus={() => focus.hoverNote(note.id)}
                onBlur={() => focus.hoverNote(null)}
              >
                {card(note)}
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  )
}
