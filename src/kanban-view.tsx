import type { DragEvent, ReactNode } from "react"
import type { Note, NoteType } from "./workspace-client"
import { noteTypeLabel } from "./note-controls"
import { kanbanColumns } from "./note-views"
import type { NoteFocus } from "./note-focus"

/**
 * The visible Notes in one column per Note Type present. The column decides
 * where a Note appears and nothing else: it places the same card the tiling
 * view places, over the same intents.
 */
export function KanbanView({
  notes,
  focus,
  card,
  onSetNoteType,
}: {
  notes: Note[]
  focus: NoteFocus
  card: (note: Note) => ReactNode
  onSetNoteType: (note: Note, noteType: NoteType) => void
}) {
  function draggedNote(event: DragEvent<HTMLDivElement>): Note | undefined {
    return notes.find((note) => note.id === event.dataTransfer.getData("application/x-nodepad-note"))
  }

  function dropNote(event: DragEvent<HTMLDivElement>, targetNoteType: NoteType) {
    event.preventDefault()
    const note = draggedNote(event)
    if (note && note.noteType !== targetNoteType) onSetNoteType(note, targetNoteType)
  }

  return (
    <div className="kanban">
      {kanbanColumns(notes).map((column) => (
        <div className="kanban-column" key={column.noteType}>
          <div className="row">
            <h3>{noteTypeLabel(column.noteType)}</h3>
            <span className="badge">{column.notes.length}</span>
          </div>
          <div
            role="group"
            aria-label={`${noteTypeLabel(column.noteType)} Notes`}
            onDragOver={(event) => event.preventDefault()}
            onDrop={(event) => dropNote(event, column.noteType)}
          >
            {column.notes.map((note) => (
              <div
                key={note.id}
                draggable
                onDragStart={(event) => {
                  event.dataTransfer.effectAllowed = "move"
                  event.dataTransfer.setData("application/x-nodepad-note", note.id)
                }}
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
