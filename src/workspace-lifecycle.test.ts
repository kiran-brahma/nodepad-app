import { describe, expect, it } from "vitest"
import type { ThinkingWorkspace } from "./workspace-client"
import {
  deleteConfirmationPrompt,
  requestDelete,
  resolveDeleteConfirmation,
} from "./workspace-lifecycle"

const workspace: ThinkingWorkspace = {
  id: "workspace-1",
  name: "Rêverie 🧠",
  assistancePolicy: "manual",
  selectedModel: null,
  cloudConsentAt: null,
  createdAt: "2026-07-22T10:00:00+00:00",
  updatedAt: "2026-07-22T10:00:00+00:00",
}

describe("delete confirmation", () => {
  it("names the Workspace it is about to delete", () => {
    expect(deleteConfirmationPrompt(requestDelete(workspace)!)).toContain("Rêverie 🧠")
  })

  it("dispatches a delete intent only after an explicit confirmation", () => {
    expect(resolveDeleteConfirmation(requestDelete(workspace), "confirm")).toEqual({
      intent: "delete",
      workspaceId: "workspace-1",
    })
  })

  it("dispatches no intent when the thinker cancels", () => {
    expect(resolveDeleteConfirmation(requestDelete(workspace), "cancel")).toEqual({
      intent: "none",
    })
  })

  it("dispatches no intent without a pending confirmation", () => {
    expect(resolveDeleteConfirmation(null, "confirm")).toEqual({ intent: "none" })
  })
})
