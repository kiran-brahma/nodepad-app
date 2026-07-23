import type { Note, NoteType } from "./workspace-client"

/** Matches the durable bound the Thinking Workspace interface enforces. */
export const MAX_ANNOTATION_SCALARS = 2000

/** A Note delete the thinker has asked for but not yet confirmed. */
export type PendingNoteDelete = { noteId: string; preview: string } | null

export type ConfirmationAnswer = "confirm" | "cancel"

export type NoteDeleteResolution = { intent: "delete"; noteId: string } | { intent: "none" }

/** The first line is enough to recognize the Note without repeating all of it. */
export function notePreview(note: Note): string {
  const firstLine = note.markdown.trim().split("\n")[0]?.trim() ?? ""
  return [...firstLine].length > 60 ? `${[...firstLine].slice(0, 60).join("")}…` : firstLine
}

export function requestNoteDelete(note: Note): NonNullable<PendingNoteDelete> {
  return { noteId: note.id, preview: notePreview(note) }
}

/** Deleting a Note is reversible this session, and the prompt says so. */
export function noteDeleteConfirmationPrompt(pending: NonNullable<PendingNoteDelete>): string {
  return `Delete “${pending.preview}”? You can undo this while Nodepad stays open.`
}

export function resolveNoteDeleteConfirmation(
  pending: PendingNoteDelete,
  answer: ConfirmationAnswer,
): NoteDeleteResolution {
  if (!pending || answer === "cancel") return { intent: "none" }
  return { intent: "delete", noteId: pending.noteId }
}

/** Only the count the durable bound uses: Unicode scalar values, not bytes. */
export function annotationLength(annotation: string): number {
  return [...annotation.trim()].length
}

export function isAnnotationTooLong(annotation: string): boolean {
  return annotationLength(annotation) > MAX_ANNOTATION_SCALARS
}

/** A Note Type reads as a word, not an identifier. */
export function noteTypeLabel(noteType: NoteType): string {
  return noteType.charAt(0).toUpperCase() + noteType.slice(1)
}

type UndoKeyEvent = {
  key: string
  metaKey: boolean
  ctrlKey: boolean
  shiftKey: boolean
  /** Undo belongs to the Notes, not to text the thinker is still typing. */
  editingText: boolean
}

/** macOS uses Command-Z; Control-Z keeps the app usable on other keyboards. */
export function isUndoShortcut(event: UndoKeyEvent): boolean {
  if (event.editingText || event.shiftKey) return false
  return event.key.toLowerCase() === "z" && (event.metaKey || event.ctrlKey)
}
