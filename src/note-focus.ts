import { useEffect, useRef, useState } from "react"

export interface NoteFocus {
  focusedNoteId: string | null
  /** Navigating to a Note only moves the reader; it commits nothing. */
  focusNote: (noteId: string) => void
  registerNoteElement: (noteId: string, element: HTMLLIElement | null) => void
}

/**
 * Which Note the thinker navigated to. Focus is transient: it is never
 * committed, and moving it can change no Relationship.
 */
export function useNoteFocus(): NoteFocus {
  const [focusedNoteId, setFocusedNoteId] = useState<string | null>(null)
  const noteElements = useRef(new Map<string, HTMLLIElement>())

  useEffect(() => {
    if (!focusedNoteId) return
    const element = noteElements.current.get(focusedNoteId)
    element?.scrollIntoView?.({ block: "center" })
    element?.focus()
  }, [focusedNoteId])

  return {
    focusedNoteId,
    focusNote: setFocusedNoteId,
    registerNoteElement: (noteId, element) => {
      if (element) noteElements.current.set(noteId, element)
      else noteElements.current.delete(noteId)
    },
  }
}
