import { describe, expect, it } from "vitest"
import type { Note, ThinkingWorkspace } from "./workspace-client"
import {
  copyExplanation,
  moveExplanation,
  requestTransfer,
  transferDestination,
  transferDestinations,
} from "./note-transfer"

const workspaces: ThinkingWorkspace[] = [
  {
    id: "workspace-1",
    name: "Research",
    assistancePolicy: "manual",
    selectedModel: null,
    cloudConsentAt: null,
    createdAt: "2026-07-22T09:00:00+00:00",
    updatedAt: "2026-07-22T09:00:00+00:00",
  },
  {
    id: "workspace-2",
    name: "Reading",
    assistancePolicy: "manual",
    selectedModel: null,
    cloudConsentAt: null,
    createdAt: "2026-07-22T09:01:00+00:00",
    updatedAt: "2026-07-22T09:01:00+00:00",
  },
]

const note: Note = {
  id: "note-1",
  workspaceId: "workspace-1",
  markdown: "Cities grew around rivers\n\nA second line nobody needs here.",
  noteType: "claim",
  noteTypeProvenance: "manual",
  annotation: null,
  annotationProvenance: "default",
  createdAt: "2026-07-22T10:00:00+00:00",
  updatedAt: "2026-07-22T10:00:00+00:00",
  pinned: false,
  labels: [],
}

describe("choosing a destination", () => {
  it("never offers the Thinking Workspace the Note is already in", () => {
    expect(transferDestinations(workspaces, note).map(({ id }) => id)).toEqual(["workspace-2"])
  })

  it("preselects the first destination", () => {
    expect(requestTransfer(workspaces, note)).toEqual({
      noteId: "note-1",
      targetWorkspaceId: "workspace-2",
    })
  })

  it("offers nothing when this is the only Thinking Workspace", () => {
    expect(requestTransfer([workspaces[0]], note)).toBeNull()
  })

  it("reports no destination once the chosen Workspace is gone", () => {
    const pending = requestTransfer(workspaces, note)
    expect(transferDestination([workspaces[0]], pending)).toBeNull()
    expect(transferDestination(workspaces, pending)?.name).toBe("Reading")
  })
})

describe("saying which transfer is which", () => {
  it("tells the thinker a move takes the Note and its Relationships away", () => {
    const explanation = moveExplanation(workspaces[1], note)
    expect(explanation).toContain("Cities grew around rivers")
    expect(explanation).toContain("Reading")
    expect(explanation).toContain("leaves this Thinking Workspace")
    expect(explanation).toContain("loses its Relationships")
    // The first line names the Note; the rest of it stays out of the prompt.
    expect(explanation).not.toContain("A second line")
  })

  it("tells the thinker a copy leaves the Note where it is", () => {
    const explanation = copyExplanation(workspaces[1], note)
    expect(explanation).toContain("This Note stays here")
    expect(explanation).toContain("no Relationships")
  })
})
