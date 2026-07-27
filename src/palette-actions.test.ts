import { describe, expect, it, vi } from "vitest"
import { buildPaletteActions } from "./palette-actions"
import { NOTE_TYPES, type Note, type ThinkingWorkspace } from "./workspace-client"
import type { NoteIntents } from "./note-card"

function workspace(id: string, name: string): ThinkingWorkspace {
  return {
    id,
    name,
    assistancePolicy: "manual",
    selectedModel: null,
    cloudConsentAt: null,
    createdAt: "2026-07-22T10:00:00+00:00",
    updatedAt: "2026-07-22T10:00:00+00:00",
  }
}

function note(id: string, pinned: boolean): Note {
  return {
    id,
    workspaceId: "w1",
    markdown: id,
    noteType: "general",
    noteTypeProvenance: "default",
    annotation: null,
    annotationProvenance: "default",
    createdAt: "2026-07-22T10:01:00+00:00",
    updatedAt: "2026-07-22T10:01:00+00:00",
    pinned,
    canvasX: null,
    canvasY: null,
    labels: [],
    enrichmentRevision: 0,
    lastEnrichedAt: null,
  }
}

/** Every intent, recorded rather than run, so an action can be checked
 *  against the one it is supposed to route to. */
function recordingIntents() {
  return {
    startEdit: vi.fn(),
    startRelate: vi.fn(),
    togglePinned: vi.fn(),
    requestDelete: vi.fn(),
    setNoteType: vi.fn(),
  } as unknown as NoteIntents
}

function build(overrides: Partial<Parameters<typeof buildPaletteActions>[0]> = {}) {
  const intents = recordingIntents()
  const actions = buildPaletteActions({
    activeWorkspace: workspace("w1", "Research"),
    workspaces: [workspace("w1", "Research"), workspace("w2", "Reading")],
    selectWorkspace: vi.fn(),
    canUndo: false,
    undo: vi.fn(),
    focusCapture: vi.fn(),
    openSettings: vi.fn(),
    renameWorkspace: vi.fn(),
    deleteWorkspace: vi.fn(),
    exportMarkdown: vi.fn(),
    exportArchive: vi.fn(),
    importArchive: vi.fn(),
    setView: vi.fn(),
    setAssistancePolicy: vi.fn(),
    focusedNote: undefined,
    noteIntents: intents,
    ...overrides,
  })
  return { actions, intents }
}

function action(actions: ReturnType<typeof build>["actions"], id: string) {
  const found = actions.find((candidate) => candidate.id === id)
  if (!found) throw new Error(`No palette action ${id}`)
  return found
}

describe("palette actions", () => {
  it("offers nothing while no Thinking Workspace is active", () => {
    const { actions } = build({ activeWorkspace: undefined })
    expect(actions).toEqual([])
  })

  it("covers every arrangement, Note Type, Assistance Policy, and Workspace", () => {
    const { actions } = build()
    const ids = actions.map(({ id }) => id)
    expect(ids).toContain("view-canvas")
    expect(ids).toContain("view-kanban")
    expect(ids).toContain("view-graph")
    for (const noteType of NOTE_TYPES) expect(ids).toContain(`note-type-${noteType}`)
    expect(ids).toContain("policy-manual")
    expect(ids).toContain("policy-local_ai")
    expect(ids).toContain("policy-cloud_ai")
    expect(ids).toContain("switch-workspace-w1")
    expect(ids).toContain("switch-workspace-w2")
    // Every id is offered once, so no two entries can shadow each other.
    expect(new Set(ids).size).toBe(ids.length)
  })

  it("names a Workspace jump after the Workspace", () => {
    const { actions } = build()
    expect(action(actions, "switch-workspace-w2").label).toBe("Switch Workspace → Reading")
  })

  it("disables the focused-Note actions while nothing is focused", () => {
    const { actions, intents } = build()
    const focusedIds = ["note-edit", "note-relate", "note-pin", "note-delete", "note-type-question"]
    for (const id of focusedIds) expect(action(actions, id).disabled).toBe(true)

    // Listed but inert: running one anyway commits nothing.
    for (const id of focusedIds) action(actions, id).run()
    expect(intents.startEdit).not.toHaveBeenCalled()
    expect(intents.togglePinned).not.toHaveBeenCalled()
    expect(intents.requestDelete).not.toHaveBeenCalled()
    expect(intents.setNoteType).not.toHaveBeenCalled()
  })

  it("routes each focused-Note action to the intent the card runs", () => {
    const focused = note("n1", false)
    const { actions, intents } = build({ focusedNote: focused })

    action(actions, "note-edit").run()
    action(actions, "note-relate").run()
    action(actions, "note-pin").run()
    action(actions, "note-delete").run()
    action(actions, "note-type-question").run()

    expect(intents.startEdit).toHaveBeenCalledWith(focused)
    expect(intents.startRelate).toHaveBeenCalledWith(focused)
    expect(intents.togglePinned).toHaveBeenCalledWith(focused)
    expect(intents.requestDelete).toHaveBeenCalledWith(focused)
    expect(intents.setNoteType).toHaveBeenCalledWith(focused, "question")
  })

  it("reads the pin entry off the Note it would act on", () => {
    expect(action(build({ focusedNote: note("n1", false) }).actions, "note-pin").label).toBe(
      "Pin focused Note",
    )
    expect(action(build({ focusedNote: note("n1", true) }).actions, "note-pin").label).toBe(
      "Unpin focused Note",
    )
  })

  it("disables Undo only when there is nothing to undo", () => {
    expect(action(build().actions, "undo").disabled).toBe(true)
    expect(action(build({ canUndo: true }).actions, "undo").disabled).toBe(false)
  })

  it("runs the Workspace and capture handlers App supplies", () => {
    const focusCapture = vi.fn()
    const openSettings = vi.fn()
    const selectWorkspace = vi.fn()
    const { actions } = build({ focusCapture, openSettings, selectWorkspace })

    action(actions, "capture-note").run()
    action(actions, "open-settings").run()
    action(actions, "switch-workspace-w2").run()

    expect(focusCapture).toHaveBeenCalledOnce()
    expect(openSettings).toHaveBeenCalledOnce()
    expect(selectWorkspace).toHaveBeenCalledWith("w2")
  })
})
