import { useEffect, useRef, type FormEvent, type KeyboardEvent } from "react"
import type { ThinkingWorkspace } from "./workspace-client"

/**
 * A single-line-growing capture bar pinned to the foot of the main pane.
 *
 * - Enter (without Shift) commits the Note via the form's onSubmit.
 * - Shift+Enter inserts a newline.
 * - Empty Enter is a no-op (the submit handler checks non-empty).
 * - Escape blurs the bar.
 * - Disabled with a placeholder when there is no active Workspace.
 * - Auto-focuses after commit so the next thought can be captured immediately.
 */
export function CaptureBar({
  activeWorkspace,
  noteMarkdown,
  onNoteMarkdownChange,
  onCreateNote,
}: {
  activeWorkspace: ThinkingWorkspace | undefined
  noteMarkdown: string
  onNoteMarkdownChange: (markdown: string) => void
  onCreateNote: (event: FormEvent) => void
}) {
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  // Auto-grow the textarea to fit its content, up to a max height.
  function autoGrow() {
    const el = textareaRef.current
    if (!el) return
    el.style.height = "auto"
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`
  }

  // Grow on every value change.
  useEffect(() => {
    autoGrow()
  }, [noteMarkdown])

  // Re-focus the bar after the note is committed (markdown cleared).
  useEffect(() => {
    if (noteMarkdown === "" && activeWorkspace) {
      textareaRef.current?.focus()
    }
  }, [noteMarkdown, activeWorkspace])

  function handleKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault()
      // Submit the parent form.
      event.currentTarget.form?.requestSubmit()
    }
    if (event.key === "Escape") {
      event.preventDefault()
      textareaRef.current?.blur()
    }
  }

  const disabled = !activeWorkspace
  const placeholder = disabled
    ? "Select a Workspace to capture"
    : "Capture a thought…"

  return (
    <form className="capture-bar" onSubmit={onCreateNote}>
      <textarea
        ref={textareaRef}
        id="capture-bar"
        className="capture-bar-input"
        value={noteMarkdown}
        onChange={(event) => onNoteMarkdownChange(event.target.value)}
        onKeyDown={handleKeyDown}
        disabled={disabled}
        placeholder={placeholder}
        rows={1}
        aria-label="New Note"
      />
    </form>
  )
}
