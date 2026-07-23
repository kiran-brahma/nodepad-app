import { FormEvent, useState } from "react"

import { thinkingWorkspace, type WorkspaceOutcome } from "./workspace-client"
import { useEscape, ESCAPE_PRIORITY } from "./escape-stack"
import { useModalFocus } from "./modal-focus"

/**
 * The disclosure a thinker must read and accept before a Thinking Workspace
 * may send Note content to Ollama Cloud. Acceptance commits a per-Workspace
 * consent row; refusal closes the dialog without recording anything. The
 * bearer key itself is never sent here — that is a separate, later step.
 *
 * It is a true modal: focus is trapped while it is open and restored to the
 * control that opened it, Escape dismisses it, and the saving state is
 * announced as it happens.
 */
export function CloudConsentDialog({
  workspaceId,
  workspaceName,
  onAccepted,
  onClose,
}: {
  workspaceId: string
  workspaceName: string
  onAccepted: (outcome: WorkspaceOutcome) => void
  onClose: () => void
}) {
  const [submitting, setSubmitting] = useState(false)
  const [failure, setFailure] = useState<string | null>(null)
  const ref = useModalFocus<HTMLDivElement>(true)
  useEscape(onClose, ESCAPE_PRIORITY.modal)

  async function handleAccept(event: FormEvent) {
    event.preventDefault()
    setSubmitting(true)
    setFailure(null)
    const outcome = await thinkingWorkspace.setCloudConsent(workspaceId, true)
    if (outcome.status === "failed" || outcome.status === "unavailable") {
      const message =
        outcome.status === "unavailable"
          ? outcome.failure.message
          : outcome.failure.message
      setFailure(message)
      setSubmitting(false)
      onAccepted(outcome)
      return
    }
    setSubmitting(false)
    onAccepted(outcome)
  }

  return (
    <div className="modal-overlay" onMouseDown={(event) => {
      if (event.target === event.currentTarget) onClose()
    }}>
      <section
        ref={ref}
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-label="Cloud AI disclosure"
      >
        <h2>Use Ollama Cloud for "{workspaceName}"?</h2>
        <p>
          When this Thinking Workspace uses Cloud AI, the active Note and a
          bounded set of relevant Notes may leave your Mac and be sent to
          Ollama Cloud for inference. The bearer key is held in the macOS
          keychain; Nodepad does not log or store the key anywhere else.
        </p>
        <p>
          Cloud AI is a per-Workspace choice. Other Thinking Workspaces are not
          affected, and you can revoke consent or remove the key at any time.
        </p>
        {failure && <p role="alert">{failure}</p>}
        {submitting && (
          <p role="status" aria-live="polite">Saving consent…</p>
        )}
        <form onSubmit={handleAccept}>
          <button type="submit" disabled={submitting}>
            I understand, enable Cloud AI
          </button>
          <button type="button" onClick={onClose} disabled={submitting}>
            Cancel
          </button>
        </form>
      </section>
    </div>
  )
}
