import type { FormEvent } from "react"
import {
  thinkingWorkspace,
  type Label,
  type NoteType,
  type ThinkingWorkspace,
  type WorkspaceOutcome,
} from "./workspace-client"
import { isAnnotationTooLong, requestNoteDelete, resolveNoteDeleteConfirmation } from "./note-controls"
import { requestTransfer } from "./note-transfer"
import type { NoteDrafts } from "./note-drafts"
import type { NoteIntents } from "./note-card"

/**
 * Builds the one set of Note intents the application has. Every view draws
 * the same card and hands it this object, so what editing, relating, moving,
 * or deleting a Note means is decided here and nowhere else.
 */
export function buildNoteIntents({
  drafts,
  workspaces,
  submit,
  focusNote,
  startLabelRename,
}: {
  drafts: NoteDrafts
  workspaces: ThinkingWorkspace[]
  submit: (pending: Promise<WorkspaceOutcome>) => Promise<boolean>
  focusNote: (noteId: string) => void
  startLabelRename: (label: Label) => void
}): NoteIntents {
  function saveText(event: FormEvent) {
    event.preventDefault()
    const draft = drafts.noteDraft
    if (!draft) return
    void submit(thinkingWorkspace.editNoteText(draft.id, draft.markdown)).then((committed) => {
      if (committed) drafts.setNoteDraft(null)
    })
  }

  function saveAnnotation(event: FormEvent) {
    event.preventDefault()
    const draft = drafts.annotationDraft
    if (!draft || isAnnotationTooLong(draft.text)) return
    void submit(thinkingWorkspace.setNoteAnnotation(draft.id, draft.text)).then((committed) => {
      if (committed) drafts.setAnnotationDraft(null)
    })
  }

  function saveLabel(event: FormEvent) {
    event.preventDefault()
    const draft = drafts.labelDraft
    if (!draft) return
    void submit(thinkingWorkspace.attachLabel(draft.noteId, draft.name)).then((committed) => {
      if (committed) drafts.setLabelDraft(null)
    })
  }

  function answerDelete(answer: "confirm" | "cancel") {
    const resolution = resolveNoteDeleteConfirmation(drafts.pendingNoteDelete, answer)
    drafts.setPendingNoteDelete(null)
    if (resolution.intent === "none") return
    drafts.setNoteDraft(null)
    drafts.setAnnotationDraft(null)
    void submit(thinkingWorkspace.deleteNote(resolution.noteId))
  }

  // Moving and copying are separate commands with separate outcomes, so each
  // has its own button rather than one button with a hidden mode.
  function transfer(kind: "move" | "copy") {
    const pending = drafts.pendingTransfer
    if (!pending) return
    const { noteId, targetWorkspaceId } = pending
    const committing =
      kind === "move"
        ? thinkingWorkspace.moveNote(noteId, targetWorkspaceId)
        : thinkingWorkspace.copyNote(noteId, targetWorkspaceId)
    void submit(committing).then((committed) => {
      if (committed) drafts.setPendingTransfer(null)
    })
  }

  return {
    startEdit: (note) => drafts.setNoteDraft({ id: note.id, markdown: note.markdown }),
    editTextDraft: (markdown) =>
      drafts.setNoteDraft((draft) => (draft ? { ...draft, markdown } : draft)),
    saveText,
    cancelEdit: () => drafts.setNoteDraft(null),
    startAnnotation: (note) =>
      drafts.setAnnotationDraft({ id: note.id, text: note.annotation ?? "" }),
    editAnnotationDraft: (text) =>
      drafts.setAnnotationDraft((draft) => (draft ? { ...draft, text } : draft)),
    saveAnnotation,
    cancelAnnotation: () => drafts.setAnnotationDraft(null),
    setNoteType: (note, noteType: NoteType) =>
      void submit(thinkingWorkspace.setNoteType(note.id, noteType)),
    togglePinned: (note) => void submit(thinkingWorkspace.setNotePinned(note.id, !note.pinned)),
    requestDelete: (note) => drafts.setPendingNoteDelete(requestNoteDelete(note)),
    answerDelete,
    startLabel: (note) => drafts.setLabelDraft({ noteId: note.id, name: "" }),
    editLabelDraft: (name) => drafts.setLabelDraft((draft) => (draft ? { ...draft, name } : draft)),
    saveLabel,
    cancelLabel: () => drafts.setLabelDraft(null),
    detachLabel: (note, label) => void submit(thinkingWorkspace.detachLabel(note.id, label.id)),
    startLabelRename,
    removeLabel: (label) => void submit(thinkingWorkspace.removeLabel(label.id)),
    startRelate: (note) => drafts.setRelateDraft({ noteId: note.id, query: "" }),
    editRelateQuery: (query) =>
      drafts.setRelateDraft((draft) => (draft ? { ...draft, query } : draft)),
    relate: (note, otherNoteId) => {
      void submit(thinkingWorkspace.relateNotes(note.id, otherNoteId)).then((committed) => {
        if (committed) drafts.setRelateDraft(null)
      })
    },
    unrelate: (note, otherNoteId) =>
      void submit(thinkingWorkspace.unrelateNotes(note.id, otherNoteId)),
    cancelRelate: () => drafts.setRelateDraft(null),
    focusNote,
    startTransfer: (note) => drafts.setPendingTransfer(requestTransfer(workspaces, note)),
    chooseTransferTarget: (targetWorkspaceId) =>
      drafts.setPendingTransfer((pending) => (pending ? { ...pending, targetWorkspaceId } : pending)),
    transfer,
    cancelTransfer: () => drafts.setPendingTransfer(null),
  }
}
