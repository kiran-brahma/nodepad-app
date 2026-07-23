import { useState, type Dispatch, type SetStateAction } from "react"
import type { PendingNoteDelete } from "./note-controls"
import type { PendingTransfer } from "./note-transfer"

export type NoteTextDraft = { id: string; markdown: string } | null
export type AnnotationDraft = { id: string; text: string } | null
export type LabelDraft = { noteId: string; name: string } | null
export type RelateDraft = { noteId: string; query: string } | null

/**
 * Every in-progress edit of one Note. The drafts live together because they
 * are all uncommitted: nothing here has reached the Thinking Workspace, and
 * the same drafts serve whichever view is showing the Note card.
 */
export interface NoteDrafts {
  noteDraft: NoteTextDraft
  setNoteDraft: Dispatch<SetStateAction<NoteTextDraft>>
  annotationDraft: AnnotationDraft
  setAnnotationDraft: Dispatch<SetStateAction<AnnotationDraft>>
  labelDraft: LabelDraft
  setLabelDraft: Dispatch<SetStateAction<LabelDraft>>
  relateDraft: RelateDraft
  setRelateDraft: Dispatch<SetStateAction<RelateDraft>>
  pendingTransfer: PendingTransfer
  setPendingTransfer: Dispatch<SetStateAction<PendingTransfer>>
  pendingNoteDelete: PendingNoteDelete
  setPendingNoteDelete: Dispatch<SetStateAction<PendingNoteDelete>>
}

export function useNoteDrafts(): NoteDrafts {
  const [noteDraft, setNoteDraft] = useState<NoteTextDraft>(null)
  const [annotationDraft, setAnnotationDraft] = useState<AnnotationDraft>(null)
  const [labelDraft, setLabelDraft] = useState<LabelDraft>(null)
  const [relateDraft, setRelateDraft] = useState<RelateDraft>(null)
  const [pendingTransfer, setPendingTransfer] = useState<PendingTransfer>(null)
  const [pendingNoteDelete, setPendingNoteDelete] = useState<PendingNoteDelete>(null)
  return {
    noteDraft,
    setNoteDraft,
    annotationDraft,
    setAnnotationDraft,
    labelDraft,
    setLabelDraft,
    relateDraft,
    setRelateDraft,
    pendingTransfer,
    setPendingTransfer,
    pendingNoteDelete,
    setPendingNoteDelete,
  }
}
