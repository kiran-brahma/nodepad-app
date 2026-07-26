import { useEffect, useRef, useState, type PointerEvent, type ReactNode } from "react"
import type { Note } from "./workspace-client"

export const CANVAS_CARD_WIDTH = 208
export const CANVAS_CARD_HEIGHT = 180
export const CANVAS_GAP = 28
export const CANVAS_COLUMNS = 4

type Position = { x: number; y: number }

/**
 * Gives Notes without a committed position deterministic free grid cells. The
 * caller commits these exactly once; this helper never owns durable state.
 */
export function autoCanvasPositions(notes: Note[]): Map<string, Position> {
  const occupied: Position[] = notes
    .filter((note) => note.canvasX !== null && note.canvasY !== null)
    .map((note) => ({ x: note.canvasX!, y: note.canvasY! }))
  const overlaps = (candidate: Position, other: Position) =>
    candidate.x < other.x + CANVAS_CARD_WIDTH &&
    candidate.x + CANVAS_CARD_WIDTH > other.x &&
    candidate.y < other.y + CANVAS_CARD_HEIGHT &&
    candidate.y + CANVAS_CARD_HEIGHT > other.y
  const positions = new Map<string, Position>()
  let cell = 0
  for (const note of notes) {
    if (note.canvasX !== null && note.canvasY !== null) continue
    let position: Position
    do {
      position = {
        x: CANVAS_GAP + (cell % CANVAS_COLUMNS) * (CANVAS_CARD_WIDTH + CANVAS_GAP),
        y: CANVAS_GAP + Math.floor(cell / CANVAS_COLUMNS) * (CANVAS_CARD_HEIGHT + CANVAS_GAP),
      }
      cell += 1
    } while (occupied.some((other) => overlaps(position, other)))
    occupied.push(position)
    positions.set(note.id, position)
  }
  return positions
}

type Drag = { noteId: string; offsetX: number; offsetY: number; position: Position }

/** A direct spatial projection of committed Notes; its drag state is transient. */
export function CanvasView({
  notes,
  card,
  onSetPosition,
}: {
  notes: Note[]
  card: (note: Note) => ReactNode
  onSetPosition: (noteId: string, x: number, y: number) => void
}) {
  const canvas = useRef<HTMLDivElement>(null)
  const attempted = useRef(new Set<string>())
  const [drag, setDrag] = useState<Drag | null>(null)
  const automatic = autoCanvasPositions(notes)

  useEffect(() => {
    for (const [noteId, position] of automatic) {
      if (attempted.current.has(noteId)) continue
      attempted.current.add(noteId)
      onSetPosition(noteId, position.x, position.y)
    }
  }, [automatic, onSetPosition])

  function positionFor(note: Note): Position {
    if (drag?.noteId === note.id) return drag.position
    return note.canvasX !== null && note.canvasY !== null
      ? { x: note.canvasX, y: note.canvasY }
      : automatic.get(note.id)!
  }

  function move(event: PointerEvent<HTMLDivElement>) {
    if (!drag || !canvas.current) return
    const bounds = canvas.current.getBoundingClientRect()
    const x = Math.max(0, Math.round(event.clientX - bounds.left + canvas.current.scrollLeft - drag.offsetX))
    const y = Math.max(0, Math.round(event.clientY - bounds.top + canvas.current.scrollTop - drag.offsetY))
    setDrag({ ...drag, position: { x, y } })
  }

  function drop(event: PointerEvent<HTMLDivElement>) {
    if (!drag) return
    event.currentTarget.releasePointerCapture(event.pointerId)
    const note = notes.find((candidate) => candidate.id === drag.noteId)
    setDrag(null)
    if (!note || (note.canvasX === drag.position.x && note.canvasY === drag.position.y)) return
    onSetPosition(note.id, drag.position.x, drag.position.y)
  }

  return (
    <div className="canvas" aria-label="Note canvas" ref={canvas}>
      {notes.map((note) => {
        const position = positionFor(note)
        return (
          <div
            className="canvas-note"
            data-note-id={note.id}
            key={note.id}
            onPointerDown={(event) => {
              if ((event.target as HTMLElement).closest("button, input, textarea, select, a, [role=button], [role=listbox]")) return
              const bounds = event.currentTarget.getBoundingClientRect()
              event.currentTarget.setPointerCapture(event.pointerId)
              setDrag({
                noteId: note.id,
                offsetX: event.clientX - bounds.left,
                offsetY: event.clientY - bounds.top,
                position,
              })
            }}
            onPointerMove={move}
            onPointerUp={drop}
            style={{ left: position.x, top: position.y }}
          >
            {card(note)}
          </div>
        )
      })}
    </div>
  )
}
