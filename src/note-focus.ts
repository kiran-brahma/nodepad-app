import { useEffect, useRef, useState } from "react"
import type { Note } from "./workspace-client"
import { preservedSelection } from "./note-views"

export interface NoteFocus {
  focusedNoteId: string | null
  /** Navigating to a Note only moves the reader; it commits nothing. */
  focusNote: (noteId: string) => void
  registerNoteElement: (noteId: string, element: HTMLDivElement | null) => void
}

/**
 * Which Note the thinker navigated to. Focus is transient: it is never
 * committed, and moving it can change no Relationship.
 */
export function useNoteFocus(visible: Note[]): NoteFocus {
  const [focusedNoteId, setFocusedNoteId] = useState<string | null>(null)
  const noteElements = useRef(new Map<string, HTMLDivElement>())

  useEffect(() => {
    if (!focusedNoteId) return
    const element = noteElements.current.get(focusedNoteId)
    element?.scrollIntoView?.({ block: "center" })
    element?.focus()
  }, [focusedNoteId])

  // Switching view, searching, or deleting can take the focused Note off
  // screen. Focus follows the Note while it is visible and is otherwise let go.
  useEffect(() => {
    setFocusedNoteId((current) => preservedSelection(current, visible))
  }, [visible])

  return {
    focusedNoteId,
    focusNote: setFocusedNoteId,
    registerNoteElement: (noteId, element) => {
      if (element) noteElements.current.set(noteId, element)
      else noteElements.current.delete(noteId)
    },
  }
}
