import { describe, expect, it } from "vitest"
import type { Note } from "./workspace-client"
import {
  annotationLength,
  isAnnotationTooLong,
  isUndoShortcut,
  MAX_ANNOTATION_SCALARS,
  noteDeleteConfirmationPrompt,
  noteTypeLabel,
  requestNoteDelete,
  resolveNoteDeleteConfirmation,
} from "./note-controls"

const note: Note = {
  id: "note-1",
  workspaceId: "workspace-1",
  markdown: "# Rêverie 🧠\n\nA second line nobody needs in a prompt.",
  noteType: "claim",
  noteTypeProvenance: "manual",
  annotation: null,
  annotationProvenance: "default",
  createdAt: "2026-07-22T10:00:00+00:00",
  updatedAt: "2026-07-22T10:00:00+00:00",
  pinned: false,
  labels: [],
  enrichmentRevision: 0,
  lastEnrichedAt: null,
}

describe("Note delete confirmation", () => {
  it("names the Note by its first line and promises a reversible delete", () => {
    const prompt = noteDeleteConfirmationPrompt(requestNoteDelete(note))
    expect(prompt).toContain("# Rêverie 🧠")
    expect(prompt).not.toContain("second line")
    expect(prompt).toContain("undo")
  })

  it("shortens a long first line by scalar values rather than bytes", () => {
    const long = { ...note, markdown: "🧠".repeat(80) }
    expect([...requestNoteDelete(long).preview]).toHaveLength(61)
  })

  it("dispatches a delete intent only after an explicit confirmation", () => {
    expect(resolveNoteDeleteConfirmation(requestNoteDelete(note), "confirm")).toEqual({
      intent: "delete",
      noteId: "note-1",
    })
    expect(resolveNoteDeleteConfirmation(requestNoteDelete(note), "cancel")).toEqual({
      intent: "none",
    })
    expect(resolveNoteDeleteConfirmation(null, "confirm")).toEqual({ intent: "none" })
  })
})

describe("Annotation bounds", () => {
  it("counts trimmed Unicode scalar values, matching the durable bound", () => {
    expect(annotationLength("  🧠🧠  ")).toBe(2)
    expect(isAnnotationTooLong("🧠".repeat(MAX_ANNOTATION_SCALARS))).toBe(false)
    expect(isAnnotationTooLong("🧠".repeat(MAX_ANNOTATION_SCALARS + 1))).toBe(true)
  })
})

describe("Note Type labels", () => {
  it("reads as a word", () => {
    expect(noteTypeLabel("question")).toBe("Question")
  })
})

describe("undo shortcut", () => {
  const shortcut = { key: "z", metaKey: true, ctrlKey: false, shiftKey: false, editingText: false }

  it("accepts Command-Z and Control-Z", () => {
    expect(isUndoShortcut(shortcut)).toBe(true)
    expect(isUndoShortcut({ ...shortcut, metaKey: false, ctrlKey: true })).toBe(true)
    expect(isUndoShortcut({ ...shortcut, key: "Z" })).toBe(true)
  })

  it("ignores redo, plain typing, and text the thinker is still writing", () => {
    expect(isUndoShortcut({ ...shortcut, shiftKey: true })).toBe(false)
    expect(isUndoShortcut({ ...shortcut, metaKey: false })).toBe(false)
    expect(isUndoShortcut({ ...shortcut, editingText: true })).toBe(false)
  })
})
