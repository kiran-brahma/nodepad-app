import { NOTE_TYPES, type AssistancePolicy, type Note, type ThinkingWorkspace } from "./workspace-client"
import { noteTypeLabel } from "./note-controls"
import type { NoteIntents } from "./note-card"
import type { NoteView } from "./note-views"
import { noteViewLabel, NOTE_VIEWS } from "./note-views"
import type { PaletteAction } from "./command-palette"

/**
 * Builds the Command-K palette's actions from handlers that already exist in
 * App, so the palette module itself never learns about Workspaces, views, or
 * assistance policy. A module-level builder keeps the branching out of the
 * App component body. Returns nothing when there is no active Workspace.
 *
 * Every action the redesign added is reachable here — view, capture,
 * settings, Workspace switching, and the focused Note's own controls — so the
 * whole application is drivable from the keyboard. The focused-Note actions
 * are routed through the same intents object the card uses, so ⌘K can never
 * mean something different from clicking the card.
 */
export function buildPaletteActions(input: {
  activeWorkspace: ThinkingWorkspace | undefined
  /** Every Thinking Workspace, so ⌘K can jump to one by name. */
  workspaces: ThinkingWorkspace[]
  selectWorkspace: (workspaceId: string) => void
  canUndo: boolean
  undo: () => void
  focusCapture: () => void
  openSettings: () => void
  renameWorkspace: () => void
  deleteWorkspace: () => void
  exportMarkdown: () => void
  exportArchive: () => void
  importArchive: () => void
  setView: (view: NoteView) => void
  setAssistancePolicy: (policy: AssistancePolicy) => void
  /** The Note the thinker is reading, if any. Its actions are listed either
   *  way, disabled while nothing is focused, so the palette reads the same
   *  from anywhere. */
  focusedNote: Note | undefined
  noteIntents: NoteIntents
}): PaletteAction[] {
  if (!input.activeWorkspace) return []
  return [
    { id: "capture-note", label: "Capture a thought", group: "Capture", run: input.focusCapture },
    { id: "undo", label: "Undo", group: "Notes", disabled: !input.canUndo, run: input.undo },
    ...focusedNoteActions(input.focusedNote, input.noteIntents),
    { id: "open-settings", label: "Open Workspace settings", group: "Workspace", run: input.openSettings },
    { id: "rename-workspace", label: "Rename Workspace", group: "Workspace", run: input.renameWorkspace },
    { id: "delete-workspace", label: "Delete Workspace", group: "Workspace", run: input.deleteWorkspace },
    { id: "export-markdown", label: "Export Markdown", group: "Workspace", run: input.exportMarkdown },
    { id: "export-archive", label: "Export Archive", group: "Workspace", run: input.exportArchive },
    { id: "import-archive", label: "Import Archive", group: "Workspace", run: input.importArchive },
    ...NOTE_VIEWS.map((view) => ({
      id: `view-${view}`,
      label: `${noteViewLabel(view)} view`,
      group: "View",
      run: () => input.setView(view),
    })),
    { id: "policy-manual", label: "Assistance: Manual", group: "Assistance", run: () => input.setAssistancePolicy("manual") },
    { id: "policy-local", label: "Assistance: Local AI", group: "Assistance", run: () => input.setAssistancePolicy("local_ai") },
    { id: "policy-cloud", label: "Assistance: Cloud AI", group: "Assistance", run: () => input.setAssistancePolicy("cloud_ai") },
    // One jump per Thinking Workspace, matched by name, running the same
    // switch the rail row runs.
    ...input.workspaces.map((workspace) => ({
      id: `switch-workspace-${workspace.id}`,
      label: `Switch Workspace → ${workspace.name}`,
      group: "Switch Workspace",
      run: () => input.selectWorkspace(workspace.id),
    })),
  ]
}

/**
 * What may be done to the Note the thinker is reading. Each entry runs the
 * intent the card's own control runs; none of them commits anything the
 * palette decides. With nothing focused the entries stay listed and disabled,
 * so the thinker learns they exist rather than wondering where they went.
 */
function focusedNoteActions(note: Note | undefined, intents: NoteIntents): PaletteAction[] {
  const disabled = !note
  const withNote = (run: (note: Note) => void) => () => {
    if (note) run(note)
  }
  return [
    {
      id: "note-edit",
      label: "Edit focused Note",
      group: "Notes",
      disabled,
      run: withNote(intents.startEdit),
    },
    {
      id: "note-relate",
      label: "Relate focused Note",
      group: "Notes",
      disabled,
      run: withNote(intents.startRelate),
    },
    {
      id: "note-pin",
      // The label states what the action does to this Note, so pinning and
      // unpinning are never the same word.
      label: note?.pinned ? "Unpin focused Note" : "Pin focused Note",
      group: "Notes",
      disabled,
      run: withNote(intents.togglePinned),
    },
    {
      id: "note-delete",
      label: "Delete focused Note",
      group: "Notes",
      disabled,
      run: withNote(intents.requestDelete),
    },
    // One entry per Note Type, so setting the classification never needs a
    // second surface or the mouse.
    ...NOTE_TYPES.map((noteType) => ({
      id: `note-type-${noteType}`,
      label: `Set Note Type → ${noteTypeLabel(noteType)}`,
      group: "Note Type",
      disabled,
      run: withNote((focused) => intents.setNoteType(focused, noteType)),
    })),
  ]
}
