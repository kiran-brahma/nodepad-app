import { useEffect, useRef, useState, type PointerEvent, type ReactNode } from "react"
import type { Note } from "./workspace-client"
import type { ThinkingGraph } from "./thinking-graph"
import type { NoteFocus } from "./note-focus"
import type { SuggestedRelationship } from "./suggested-relationships"

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
type RelationshipDrag = { sourceNoteId: string; pointerId: number }

export type CanvasRelationship = {
  id: string
  noteIdA: string
  noteIdB: string
  source: Position
  target: Position
  focused: boolean
}

/**
 * The canvas reads its lines from the one shared Thinking Graph projection.
 * A link without two displayed endpoints has no line to draw, never a second
 * relationship list invented by this view.
 */
export function canvasRelationships(
  graph: ThinkingGraph,
  positions: ReadonlyMap<string, Position>,
  litNoteIds: ReadonlySet<string> | null,
): CanvasRelationship[] {
  return graph.links.flatMap((link) => {
    const source = positions.get(link.noteIdA)
    const target = positions.get(link.noteIdB)
    if (!source || !target) return []
    return [{
      id: link.id,
      noteIdA: link.noteIdA,
      noteIdB: link.noteIdB,
      source,
      target,
      focused: litNoteIds !== null && (litNoteIds.has(link.noteIdA) || litNoteIds.has(link.noteIdB)),
    }]
  })
}

type SuggestedCanvasLine = {
  key: string
  source: Position
  target: Position
}

/**
 * The lines for suggestions the thinker has not answered. They are drawn from
 * the same positions as committed Relationships and nothing else: a proposal
 * is not in the Thinking Graph, so it can only ever be dashed.
 */
function suggestedCanvasLines(
  suggestions: readonly SuggestedRelationship[],
  positions: ReadonlyMap<string, Position>,
): SuggestedCanvasLine[] {
  return suggestions.flatMap((suggestion) => {
    const source = positions.get(suggestion.noteId)
    const target = positions.get(suggestion.otherNoteId)
    if (!source || !target) return []
    return [{ key: suggestion.key, source, target }]
  })
}

/** A direct spatial projection of committed Notes; its drag state is transient. */
export function CanvasView({
  notes,
  graph,
  focus,
  card,
  suggestions,
  onSetPosition,
  onRelate,
  onUnrelate,
}: {
  notes: Note[]
  graph: ThinkingGraph
  focus: NoteFocus
  card: (note: Note) => ReactNode
  /** Undecided AI proposals, drawn dashed. They commit nothing and are not
   *  part of the Thinking Graph. */
  suggestions: readonly SuggestedRelationship[]
  onSetPosition: (noteId: string, x: number, y: number) => void
  onRelate: (noteId: string, otherNoteId: string) => void
  onUnrelate: (noteId: string, otherNoteId: string) => void
}) {
  const canvas = useRef<HTMLDivElement>(null)
  const attempted = useRef(new Set<string>())
  const [drag, setDrag] = useState<Drag | null>(null)
  const [relationshipDrag, setRelationshipDrag] = useState<RelationshipDrag | null>(null)
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
    event.currentTarget.releasePointerCapture?.(event.pointerId)
    const note = notes.find((candidate) => candidate.id === drag.noteId)
    setDrag(null)
    if (!note || (note.canvasX === drag.position.x && note.canvasY === drag.position.y)) return
    onSetPosition(note.id, drag.position.x, drag.position.y)
  }

  function relationshipTarget(event: PointerEvent<HTMLDivElement>): string | null {
    const underPointer = document.elementFromPoint?.(event.clientX, event.clientY)
    const target = underPointer ?? event.target
    return target instanceof Element ? target.closest<HTMLElement>("[data-note-id]")?.dataset.noteId ?? null : null
  }

  function finishLink(event: PointerEvent<HTMLDivElement>) {
    if (!relationshipDrag) return
    if (event.pointerId !== relationshipDrag.pointerId) return
    const captured = event.target
    if (captured instanceof Element && captured.hasPointerCapture?.(event.pointerId)) {
      captured.releasePointerCapture(event.pointerId)
    }
    const targetNoteId = relationshipTarget(event)
    setRelationshipDrag(null)
    if (!targetNoteId || targetNoteId === relationshipDrag.sourceNoteId) return
    onRelate(relationshipDrag.sourceNoteId, targetNoteId)
  }

  const positions = new Map(notes.map((note) => [note.id, positionFor(note)]))
  const relationships = canvasRelationships(graph, positions, focus.litNoteIds)
  const suggestedLines = suggestedCanvasLines(suggestions, positions)

  return (
    <div className="canvas" aria-label="Note canvas" ref={canvas} onPointerUp={finishLink}>
      <svg className="canvas-relationships" aria-label="Relationships">
        {/* A proposal, drawn dashed so it never reads as a link the Thinking
            Workspace holds. It is answered on the Note, not here. */}
        {suggestedLines.map((line) => (
          <line
            aria-hidden="true"
            className="canvas-relationship suggested"
            data-suggested-relationship={line.key}
            key={line.key}
            x1={line.source.x + CANVAS_CARD_WIDTH / 2}
            x2={line.target.x + CANVAS_CARD_WIDTH / 2}
            y1={line.source.y + CANVAS_CARD_HEIGHT / 2}
            y2={line.target.y + CANVAS_CARD_HEIGHT / 2}
          />
        ))}
        {relationships.map((relationship) => (
          <line
            aria-label="Remove Relationship"
            className={relationship.focused ? "canvas-relationship focused" : "canvas-relationship"}
            data-relationship-id={relationship.id}
            key={relationship.id}
            onClick={() => onUnrelate(relationship.noteIdA, relationship.noteIdB)}
            onKeyDown={(event) => {
              if (event.key !== "Enter" && event.key !== " ") return
              event.preventDefault()
              onUnrelate(relationship.noteIdA, relationship.noteIdB)
            }}
            role="button"
            tabIndex={0}
            x1={relationship.source.x + CANVAS_CARD_WIDTH / 2}
            x2={relationship.target.x + CANVAS_CARD_WIDTH / 2}
            y1={relationship.source.y + CANVAS_CARD_HEIGHT / 2}
            y2={relationship.target.y + CANVAS_CARD_HEIGHT / 2}
          />
        ))}
      </svg>
      {notes.map((note) => {
        const position = positionFor(note)
        return (
          <div
            className="canvas-note"
            data-note-id={note.id}
            key={note.id}
            onMouseEnter={() => focus.hoverNote(note.id)}
            onMouseLeave={() => focus.hoverNote(null)}
            onFocus={() => focus.hoverNote(note.id)}
            onBlur={() => focus.hoverNote(null)}
            onPointerDown={(event) => {
              if ((event.target as HTMLElement).closest("button, input, textarea, select, a, [role=button], [role=listbox]")) return
              const bounds = event.currentTarget.getBoundingClientRect()
              event.currentTarget.setPointerCapture?.(event.pointerId)
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
            <button
              aria-label={`Create Relationship from ${note.markdown}`}
              className="canvas-relationship-handle"
              onPointerDown={(event) => {
                event.stopPropagation()
                event.currentTarget.setPointerCapture?.(event.pointerId)
                setRelationshipDrag({ sourceNoteId: note.id, pointerId: event.pointerId })
              }}
              type="button"
            >
              Relate
            </button>
            {card(note)}
          </div>
        )
      })}
    </div>
  )
}
