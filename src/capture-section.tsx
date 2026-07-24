import type { FormEvent, ReactNode } from "react"
import type { ThinkingWorkspace } from "./workspace-client"
import { deleteConfirmationPrompt, type PendingDelete } from "./workspace-lifecycle"
import { useEscape, ESCAPE_PRIORITY } from "./escape-stack"
import { EscapeDismiss } from "./escape-dismiss"

function EscapeForm({
  onSubmit,
  onEscape,
  children,
}: {
  onSubmit: (event: FormEvent) => void
  onEscape: () => void
  children: ReactNode
}) {
  useEscape(onEscape, ESCAPE_PRIORITY.dialog)
  return <form onSubmit={onSubmit}>{children}</form>
}

/**
 * The active Thinking Workspace's admin controls — rename, delete, export,
 * and import. Rendered in the top bar of the main pane. The note capture
 * form has moved to the persistent CaptureBar in the footer.
 */
export function CaptureSection({
  activeWorkspace,
  renameDraft,
  pendingDelete,
  onStartRename,
  onRenameDraftChange,
  onRename,
  onCancelRename,
  onRequestDelete,
  onAnswerDelete,
  onExport,
  onExportArchive,
  onImportArchive,
}: {
  activeWorkspace: ThinkingWorkspace | undefined
  renameDraft: { id: string; name: string } | null
  pendingDelete: PendingDelete
  onStartRename: (workspace: ThinkingWorkspace) => void
  onRenameDraftChange: (name: string) => void
  onRename: (event: FormEvent) => void
  onCancelRename: () => void
  onRequestDelete: (workspace: ThinkingWorkspace) => void
  onAnswerDelete: (answer: "confirm" | "cancel") => void
  onExport: () => void
  onExportArchive: () => void
  onImportArchive: () => void
}) {
  return (
    <section className="capture">
      <div className="row">
        <h2>{activeWorkspace?.name ?? "Loading…"}</h2>
        {activeWorkspace && !renameDraft && (
          <div className="row">
            <button onClick={() => onStartRename(activeWorkspace)}>Rename</button>
            <button onClick={onExport}>Export Markdown</button>
            <button onClick={onExportArchive}>Export Archive</button>
            <button onClick={onImportArchive}>Import Archive</button>
            <button onClick={() => onRequestDelete(activeWorkspace)}>Delete</button>
          </div>
        )}
      </div>

      {renameDraft && (
        <EscapeForm onSubmit={onRename} onEscape={onCancelRename}>
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
        </EscapeForm>
      )}

      {pendingDelete && (
        <div className="confirm" role="alertdialog" aria-label="Confirm delete">
          {/* Escape cancels a delete the same as the Keep it button. */}
          <EscapeDismiss onEscape={() => onAnswerDelete("cancel")} />
          <p>{deleteConfirmationPrompt(pendingDelete)}</p>
          <div className="row">
            <button onClick={() => onAnswerDelete("confirm")}>Delete Workspace</button>
            <button onClick={() => onAnswerDelete("cancel")}>Keep it</button>
          </div>
        </div>
      )}
    </section>
  )
}
