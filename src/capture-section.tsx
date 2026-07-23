import type { FormEvent } from "react"
import type { ThinkingWorkspace } from "./workspace-client"
import { deleteConfirmationPrompt, type PendingDelete } from "./workspace-lifecycle"

/**
 * The active Thinking Workspace's own controls — rename, delete, and the one
 * form that captures an atomic Note into it.
 */
export function CaptureSection({
  activeWorkspace,
  renameDraft,
  pendingDelete,
  noteMarkdown,
  onStartRename,
  onRenameDraftChange,
  onRename,
  onCancelRename,
  onRequestDelete,
  onAnswerDelete,
  onNoteMarkdownChange,
  onCreateNote,
  onExport,
}: {
  activeWorkspace: ThinkingWorkspace | undefined
  renameDraft: { id: string; name: string } | null
  pendingDelete: PendingDelete
  noteMarkdown: string
  onStartRename: (workspace: ThinkingWorkspace) => void
  onRenameDraftChange: (name: string) => void
  onRename: (event: FormEvent) => void
  onCancelRename: () => void
  onRequestDelete: (workspace: ThinkingWorkspace) => void
  onAnswerDelete: (answer: "confirm" | "cancel") => void
  onNoteMarkdownChange: (markdown: string) => void
  onCreateNote: (event: FormEvent) => void
  onExport: () => void
}) {
  return (
    <section className="capture">
      <div className="row">
        <h2>{activeWorkspace?.name ?? "Loading…"}</h2>
        {activeWorkspace && !renameDraft && (
          <div className="row">
            <button onClick={() => onStartRename(activeWorkspace)}>Rename</button>
            <button onClick={onExport}>Export Markdown</button>
            <button onClick={() => onRequestDelete(activeWorkspace)}>Delete</button>
          </div>
        )}
      </div>

      {renameDraft && (
        <form onSubmit={onRename}>
          <label htmlFor="workspace-name">Thinking Workspace name</label>
          <input
            autoFocus
            id="workspace-name"
            value={renameDraft.name}
            onChange={(event) => onRenameDraftChange(event.target.value)}
          />
          <div className="row">
            <button type="submit">Save name</button>
            <button type="button" onClick={onCancelRename}>Cancel</button>
          </div>
        </form>
      )}

      {pendingDelete && (
        <div className="confirm" role="alertdialog" aria-label="Confirm delete">
          <p>{deleteConfirmationPrompt(pendingDelete)}</p>
          <div className="row">
            <button onClick={() => onAnswerDelete("confirm")}>Delete Workspace</button>
            <button onClick={() => onAnswerDelete("cancel")}>Keep it</button>
          </div>
        </div>
      )}

      <form onSubmit={onCreateNote}>
        <label htmlFor="note">New Note</label>
        <textarea id="note" value={noteMarkdown} onChange={(event) => onNoteMarkdownChange(event.target.value)} placeholder="Write an atomic Markdown Note…" rows={5} />
        <button type="submit" disabled={!activeWorkspace}>Commit Note</button>
      </form>
    </section>
  )
}
