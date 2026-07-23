import { notePreview } from "./note-controls"
import type { Note, PendingSynthesis } from "./workspace-client"
import type { SynthesisStatus } from "./synthesis-controller"

/**
 * The provisional Syntheses of the active Thinking Workspace.
 *
 * Nothing here is a Note. A pending Synthesis is shown beside its sources
 * and waits: Nodepad never accepts one on the thinker's behalf, and never
 * writes a Relationship for one. Accepting creates a fresh thesis Note;
 * dismissing removes the pending item and keeps only its text, so the next
 * attempt does not propose the same insight again.
 *
 * A Synthesis whose sources have changed cannot be accepted — the material
 * it was built from is gone — but it can always be dismissed.
 */
export function SynthesisSection({
  pending,
  notes,
  status,
  aiEnabled,
  onAccept,
  onDismiss,
}: {
  pending: PendingSynthesis[]
  notes: Note[]
  status: SynthesisStatus
  aiEnabled: boolean
  onAccept: (synthesisId: string) => void
  onDismiss: (synthesisId: string) => void
}) {
  // A Manual Workspace never asks for a Synthesis, so it is not offered a
  // panel that could only ever be empty.
  if (!aiEnabled && pending.length === 0) return null

  const sourceLabel = (noteId: string) => {
    const note = notes.find((candidate) => candidate.id === noteId)
    return note ? notePreview(note) : "A Note that is no longer here"
  }

  return (
    <section aria-label="Syntheses">
      <h2>Syntheses</h2>
      <p>{statusMessage(status, pending.length)}</p>

      {pending.length > 0 && (
        <ul aria-label="Pending Syntheses">
          {pending.map((synthesis) => (
            <li key={synthesis.id} className={synthesis.stale ? "synthesis stale" : "synthesis"}>
              <p>{synthesis.text}</p>
              <p className="provenance">
                Suggested by {synthesis.model}. Nothing is committed until you accept it.
              </p>
              {synthesis.labels.length > 0 && (
                <ul aria-label="Suggested Labels">
                  {synthesis.labels.map((label) => (
                    <li key={label}>{label}</li>
                  ))}
                </ul>
              )}
              <ul aria-label="Source Notes">
                {synthesis.sourceNoteIds.map((noteId) => (
                  <li key={noteId}>{sourceLabel(noteId)}</li>
                ))}
              </ul>
              {synthesis.stale && (
                <p role="status">
                  The Notes behind this Synthesis have changed. Dismiss it and Nodepad will look
                  again.
                </p>
              )}
              <div className="row">
                <button
                  disabled={synthesis.stale}
                  onClick={() => onAccept(synthesis.id)}
                  title="Create a new thesis Note from this Synthesis"
                >
                  Accept as a Note
                </button>
                <button onClick={() => onDismiss(synthesis.id)}>Dismiss</button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}

/**
 * One calm sentence about the last attempt. Finding no Synthesis is a
 * successful outcome, and so is being ineligible, so neither is rendered as
 * an alert; only a real provider or schema failure is.
 */
function statusMessage(status: SynthesisStatus, pendingCount: number): string {
  switch (status.kind) {
    case "debouncing":
    case "in_flight":
      return "Looking for a Synthesis across these Notes…"
    case "no_insight":
      return "Nothing worth proposing yet. Nodepad will look again as this Workspace grows."
    case "ineligible":
      return status.message
    case "failed":
      return `Synthesis could not run: ${status.message}`
    case "proposed":
    case "idle":
      return pendingCount === 0
        ? "No Synthesis is waiting on you."
        : `${pendingCount} Synthesis${pendingCount === 1 ? "" : "es"} waiting on you.`
  }
}
