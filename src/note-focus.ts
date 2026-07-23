import { useEffect, useMemo, useRef, useState } from "react"
import type { Note } from "./workspace-client"
import { litNoteIds, type ThinkingGraph } from "./thinking-graph"
import { preservedSelection } from "./note-views"
import { useEscape, ESCAPE_PRIORITY } from "./escape-stack"

export interface NoteFocus {
  /** The Note the thinker locked focus on, by clicking it or navigating to it. */
  focusedNoteId: string | null
  /**
   * The Notes the current focus lights: the focal Note and everything related
   * to it. Null while nothing is focused, which every view reads as dimming
   * nothing. One set, so no two views can dim differently.
   */
  litNoteIds: ReadonlySet<string> | null
  /** Navigating to a Note only moves the reader; it commits nothing. */
  focusNote: (noteId: string) => void
  /** Clicking the focused Note again lets it go. */
  toggleFocus: (noteId: string) => void
  /** Previewing a Note, which outlives no pointer move. */
  hoverNote: (noteId: string | null) => void
  registerNoteElement: (noteId: string, element: HTMLDivElement | null) => void
}

/**
 * Which Note the thinker is reading, and what that lights. Focus is transient:
 * it is never committed, and neither hovering nor locking can change a
 * Relationship. A hover previews; a click locks until it is clicked again,
 * Escape is pressed, or the Note leaves the screen.
 */
export function useNoteFocus(visible: Note[], graph: ThinkingGraph): NoteFocus {
  const [focusedNoteId, setFocusedNoteId] = useState<string | null>(null)
  const [hoveredNoteId, setHoveredNoteId] = useState<string | null>(null)
  const noteElements = useRef(new Map<string, HTMLDivElement>())

  useEffect(() => {
    if (!focusedNoteId) return
    const element = noteElements.current.get(focusedNoteId)
    element?.scrollIntoView?.({ block: "center" })
    element?.focus()
  }, [focusedNoteId])

  // Switching view, searching, or deleting can take a Note off screen. Focus
  // follows a Note while it is visible and is otherwise let go.
  useEffect(() => {
    setFocusedNoteId((current) => preservedSelection(current, visible))
    setHoveredNoteId((current) => preservedSelection(current, visible))
  }, [visible])

  // Escape lets go of the lock wherever the thinker is reading. It is the
  // lowest-priority dismissible surface, so an open modal or inline draft
  // wins the key first; the coordinator in `escape-stack` owns the order.
  useEscape(() => {
    setFocusedNoteId(null)
    setHoveredNoteId(null)
  }, ESCAPE_PRIORITY.focus)

  // A hover previews over whatever is locked, so there is no mode to be in.
  const focalNoteId = hoveredNoteId ?? focusedNoteId
  const lit = useMemo(() => litNoteIds(graph, focalNoteId), [graph, focalNoteId])

  return {
    focusedNoteId,
    litNoteIds: lit,
    focusNote: setFocusedNoteId,
    toggleFocus: (noteId) =>
      setFocusedNoteId((current) => (current === noteId ? null : noteId)),
    hoverNote: setHoveredNoteId,
    registerNoteElement: (noteId, element) => {
      if (element) noteElements.current.set(noteId, element)
      else noteElements.current.delete(noteId)
    },
  }
}
