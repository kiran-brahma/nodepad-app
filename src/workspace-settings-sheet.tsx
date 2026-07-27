import type { FormEvent } from "react"
import type {
  AssistancePolicy,
  CloudProvider,
  DiscoveryState,
  ThinkingWorkspace,
} from "./workspace-client"
import { AssistanceSection } from "./assistance-section"
import { deleteConfirmationPrompt, type PendingDelete } from "./workspace-lifecycle"
import { useEscape, ESCAPE_PRIORITY } from "./escape-stack"
import { useModalFocus } from "./modal-focus"
import { WorkspaceRenameForm, type WorkspaceRenameDraft } from "./workspace-rename-form"

/**
 * A settings sheet (modal dialog) for the active Thinking Workspace. Hosts
 * Assistance Policy, model discovery, cloud key & consent, export/import,
 * and rename/delete — moved out of the main pane so only Notes and capture
 * compete for the thinker's attention.
 *
 * Focus-trapped like RenameLabelModal. Escape and scrim click close it.
 */
export function WorkspaceSettingsSheet({
  activeWorkspace,
  renameDraft,
  pendingDelete,
  localState,
  localQuery,
  localFilteredModels,
  cloudState,
  cloudQuery,
  cloudFilteredModels,
  cloudKeyPresent,
  selectedMissing,
  onClose,
  onStartRename,
  onRenameDraftChange,
  onRename,
  onCancelRename,
  onRequestDelete,
  onAnswerDelete,
  onExport,
  onExportArchive,
  onImportArchive,
  onPolicyChange,
  onCloudProviderChange,
  onLocalQueryChange,
  onLocalRefresh,
  onCloudQueryChange,
  onCloudRefresh,
  onCloudKeyChange,
  onRequestCloudConsent,
  onRevokeCloudConsent,
  onSelectModel,
}: {
  activeWorkspace: ThinkingWorkspace | undefined
  renameDraft: WorkspaceRenameDraft | null
  pendingDelete: PendingDelete
  localState: DiscoveryState
  localQuery: string
  localFilteredModels: string[]
  cloudState: DiscoveryState
  cloudQuery: string
  cloudFilteredModels: string[]
  cloudKeyPresent: boolean
  selectedMissing: boolean
  onClose: () => void
  onStartRename: (workspace: ThinkingWorkspace) => void
  onRenameDraftChange: (name: string) => void
  onRename: (event: FormEvent) => void
  onCancelRename: () => void
  onRequestDelete: (workspace: ThinkingWorkspace) => void
  onAnswerDelete: (answer: "confirm" | "cancel") => void
  onExport: () => void
  onExportArchive: () => void
  onImportArchive: () => void
  onPolicyChange: (policy: AssistancePolicy) => void
  onCloudProviderChange: (provider: CloudProvider) => void
  onLocalQueryChange: (query: string) => void
  onLocalRefresh: () => void
  onCloudQueryChange: (query: string) => void
  onCloudRefresh: () => void
  onCloudKeyChange: () => void
  onRequestCloudConsent: () => void
  onRevokeCloudConsent: () => void
  onSelectModel: (modelId: string) => void
}) {
  const ref = useModalFocus<HTMLDivElement>(true)
  useEscape(onClose, ESCAPE_PRIORITY.modal)

  return (
    <div className="modal-overlay" onMouseDown={(event) => {
      if (event.target === event.currentTarget) onClose()
    }}>
      <section
        ref={ref}
        className="modal settings-sheet"
        role="dialog"
        aria-modal="true"
        aria-label="Workspace settings"
      >
        <div className="settings-sheet-header">
          <h2>{activeWorkspace?.name ?? "Workspace"} settings</h2>
          <button onClick={onClose} aria-label="Close settings">Close</button>
        </div>

        <div className="settings-sheet-body">
          {/* Workspace name and rename */}
          <section aria-label="Workspace name">
            <h3>Workspace name</h3>
            {renameDraft ? (
              <WorkspaceRenameForm
                draft={renameDraft}
                fieldLabel="Thinking Workspace name"
                submitLabel="Save name"
                cancelLabel="Cancel"
                onDraftChange={onRenameDraftChange}
                onSubmit={onRename}
                onCancel={onCancelRename}
              />
            ) : (
              <div className="row">
                <span>{activeWorkspace?.name}</span>
                {activeWorkspace && (
                  <button onClick={() => onStartRename(activeWorkspace)}>Rename</button>
                )}
              </div>
            )}
          </section>

          {/* Delete workspace */}
          <section aria-label="Delete workspace">
            <h3>Delete workspace</h3>
            {pendingDelete ? (
              <div className="confirm" role="alertdialog" aria-label="Confirm delete workspace">
                <p>{deleteConfirmationPrompt(pendingDelete)}</p>
                <div className="row">
                  <button onClick={() => onAnswerDelete("confirm")}>Delete Workspace</button>
                  <button onClick={() => onAnswerDelete("cancel")}>Keep it</button>
                </div>
              </div>
            ) : (
              <div>
                <p>Permanently delete this Workspace and all its Notes.</p>
                {activeWorkspace && (
                  <button onClick={() => onRequestDelete(activeWorkspace)}>Delete Workspace</button>
                )}
              </div>
            )}
          </section>

          {/* Export and import */}
          <section aria-label="Export and import">
            <h3>Export & Import</h3>
            <div className="row">
              <button onClick={onExport}>Export Markdown</button>
              <button onClick={onExportArchive}>Export Archive</button>
              <button onClick={onImportArchive}>Import Archive</button>
            </div>
          </section>

          {/* Assistance Policy, model discovery, cloud key */}
          <AssistanceSection
            activeWorkspace={activeWorkspace}
            localState={localState}
            localQuery={localQuery}
            localFilteredModels={localFilteredModels}
            cloudState={cloudState}
            cloudQuery={cloudQuery}
            cloudFilteredModels={cloudFilteredModels}
            cloudKeyPresent={cloudKeyPresent}
            selectedMissing={selectedMissing}
            onPolicyChange={onPolicyChange}
            onCloudProviderChange={onCloudProviderChange}
            onLocalQueryChange={onLocalQueryChange}
            onLocalRefresh={onLocalRefresh}
            onCloudQueryChange={onCloudQueryChange}
            onCloudRefresh={onCloudRefresh}
            onCloudKeyChange={onCloudKeyChange}
            onRequestCloudConsent={onRequestCloudConsent}
            onRevokeCloudConsent={onRevokeCloudConsent}
            onSelectModel={onSelectModel}
          />
        </div>
      </section>
    </div>
  )
}
