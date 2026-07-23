import type { Note, ThinkingWorkspace } from "./workspace-client"
import { notePreview } from "./note-controls"

/**
 * Choosing where a Note goes, and saying plainly what each of the two
 * transfers does to it. Nothing here writes: the durable rules — atomicity,
 * Label remapping, and the Relationship seam — belong to the Rust Thinking
 * Workspace interface.
 */

/** A destination the thinker has chosen but not yet moved or copied into. */
export type PendingTransfer = { noteId: string; targetWorkspaceId: string } | null

/** The Thinking Workspaces a Note may travel to: every one but its own. */
export function transferDestinations(
  workspaces: ThinkingWorkspace[],
  note: Note,
): ThinkingWorkspace[] {
  return workspaces.filter((workspace) => workspace.id !== note.workspaceId)
}

/**
 * Opens the destination choice with the first destination already selected, or
 * nothing at all when this is the only Thinking Workspace there is.
 */
export function requestTransfer(
  workspaces: ThinkingWorkspace[],
  note: Note,
): PendingTransfer {
  const [destination] = transferDestinations(workspaces, note)
  return destination ? { noteId: note.id, targetWorkspaceId: destination.id } : null
}

/** The chosen destination, or nothing when it no longer exists. */
export function transferDestination(
  workspaces: ThinkingWorkspace[],
  pending: PendingTransfer,
): ThinkingWorkspace | null {
  return workspaces.find((workspace) => workspace.id === pending?.targetWorkspaceId) ?? null
}

/**
 * What moving does, in the thinker's terms: one Note that changes address and
 * loses its Relationships, because a Relationship stays inside one Workspace.
 */
export function moveExplanation(destination: ThinkingWorkspace, note: Note): string {
  return `Move “${notePreview(note)}” to ${destination.name}. It leaves this Thinking Workspace, keeps its Labels by name, and loses its Relationships.`
}

/** What copying does: a second Note, with nothing tying it back. */
export function copyExplanation(destination: ThinkingWorkspace, note: Note): string {
  return `Copy “${notePreview(note)}” to ${destination.name}. This Note stays here, and the copy arrives with its Labels but no Relationships.`
}
