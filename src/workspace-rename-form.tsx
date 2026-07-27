import type { FormEvent } from "react"
import { useEscape, ESCAPE_PRIORITY } from "./escape-stack"

/** A Thinking Workspace name the thinker is editing but has not committed.
 *  Transient per surface: the rail row and the settings sheet each hold their
 *  own, and both commit through the one `renameWorkspace`. */
export interface WorkspaceRenameDraft {
  id: string
  name: string
}

/**
 * The one in-place rename form. The rail row and the settings sheet render it
 * with their own draft and their own accessible name, so the two surfaces read
 * differently but behave identically: Escape cancels through the shared escape
 * stack, and submitting runs the caller's `renameWorkspace`.
 */
export function WorkspaceRenameForm({
  draft,
  fieldLabel,
  submitLabel,
  onDraftChange,
  onSubmit,
  onCancel,
  cancelLabel,
}: {
  draft: WorkspaceRenameDraft
  /** What the field is called, so two open surfaces stay addressable. */
  fieldLabel: string
  submitLabel: string
  onDraftChange: (name: string) => void
  onSubmit: (event: FormEvent) => void
  onCancel: () => void
  /** A visible cancel button, where the surface has room for one. */
  cancelLabel?: string
}) {
  useEscape(onCancel, ESCAPE_PRIORITY.dialog)
  return (
    <form onSubmit={onSubmit}>
      <input
        autoFocus
        aria-label={fieldLabel}
        value={draft.name}
        onChange={(event) => onDraftChange(event.target.value)}
      />
      <div className="row">
        <button type="submit">{submitLabel}</button>
        {cancelLabel && (
          <button type="button" onClick={onCancel}>
            {cancelLabel}
          </button>
        )}
      </div>
    </form>
  )
}
