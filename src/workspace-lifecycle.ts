import type { ThinkingWorkspace } from "./workspace-client"

/** A delete the thinker has asked for but not yet confirmed. */
export type PendingDelete = { workspaceId: string; workspaceName: string } | null

export type ConfirmationAnswer = "confirm" | "cancel"

/** Only a confirmed delete becomes a durable intent; cancelling dispatches none. */
export type DeleteResolution =
  | { intent: "delete"; workspaceId: string }
  | { intent: "none" }

export function requestDelete(workspace: ThinkingWorkspace): PendingDelete {
  return { workspaceId: workspace.id, workspaceName: workspace.name }
}

/** Confirmation always names the Workspace being deleted. */
export function deleteConfirmationPrompt(pending: NonNullable<PendingDelete>): string {
  return `Delete “${pending.workspaceName}” and every Note in it? This cannot be undone.`
}

export function resolveDeleteConfirmation(
  pending: PendingDelete,
  answer: ConfirmationAnswer,
): DeleteResolution {
  if (!pending || answer === "cancel") return { intent: "none" }
  return { intent: "delete", workspaceId: pending.workspaceId }
}
