import { useEffect } from "react"
import { isUndoShortcut } from "./note-controls"

/**
 * Undo is a keyboard habit, so it works anywhere except inside text the
 * thinker is still writing, where the field's own undo belongs.
 */
export function useUndoShortcut(undoLastChange: () => void): void {
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      const editing = document.activeElement
      const editingText =
        editing instanceof HTMLTextAreaElement ||
        (editing instanceof HTMLInputElement && editing.type === "text")
      const shortcut = {
        key: event.key,
        metaKey: event.metaKey,
        ctrlKey: event.ctrlKey,
        shiftKey: event.shiftKey,
        editingText,
      }
      if (!isUndoShortcut(shortcut)) return
      event.preventDefault()
      undoLastChange()
    }
    window.addEventListener("keydown", onKeyDown)
    return () => window.removeEventListener("keydown", onKeyDown)
  }, [undoLastChange])
}
