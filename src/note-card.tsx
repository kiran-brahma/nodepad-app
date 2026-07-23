import type { FormEvent } from "react"
import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"
import {
  NOTE_TYPES,
  type Label,
  type Note,
  type NoteType,
  type Relationship,
  type ThinkingWorkspace,
} from "./workspace-client"
import {
  annotationLength,
  isAnnotationTooLong,
  MAX_ANNOTATION_SCALARS,
  noteDeleteConfirmationPrompt,
  notePreview,
  noteTypeLabel,
} from "./note-controls"
import { degree, relatableNotes, relatedNotes } from "./thinking-graph"
import {
  copyExplanation,
  moveExplanation,
  transferDestination,
  transferDestinations,
  type PendingTransfer,
} from "./note-transfer"
import type { NoteDrafts } from "./note-drafts"

/**
 * Every intent a Note card can raise. One object is built once, in App, and
 * handed to whichever view is showing the card, so a Note is edited, related,
 * moved, and deleted through exactly one set of handlers no matter where it
 * is drawn.
 */
export interface NoteIntents {
  startEdit: (note: Note) => void
  saveText: (event: FormEvent) => void
  cancelEdit: () => void
  startAnnotation: (note: Note) => void
  saveAnnotation: (event: FormEvent) => void
  cancelAnnotation: () => void
  setNoteType: (note: Note, noteType: NoteType) => void
  togglePinned: (note: Note) => void
  requestDelete: (note: Note) => void
  answerDelete: (answer: "confirm" | "cancel") => void
  startLabel: (note: Note) => void
  editLabelDraft: (name: string) => void
  saveLabel: (event: FormEvent) => void
  cancelLabel: () => void
  detachLabel: (note: Note, label: Label) => void
  startLabelRename: (label: Label) => void
  removeLabel: (label: Label) => void
  startRelate: (note: Note) => void
  editRelateQuery: (query: string) => void
  relate: (note: Note, otherNoteId: string) => void
  unrelate: (note: Note, otherNoteId: string) => void
  cancelRelate: () => void
  focusNote: (noteId: string) => void
  startTransfer: (note: Note) => void
  chooseTransferTarget: (targetWorkspaceId: string) => void
  transfer: (kind: "move" | "copy") => void
  cancelTransfer: () => void
  editTextDraft: (markdown: string) => void
  editAnnotationDraft: (text: string) => void
}

/**
 * The Notes and Relationships a card reads to draw itself. A card commits
 * nothing from this; it only shows what already exists.
 */
export interface NoteCardContext {
  notes: Note[]
  relationships: Relationship[]
  workspaces: ThinkingWorkspace[]
}

/**
 * The destination choice for one Note, with the two transfers named and
 * described separately so a move can never be mistaken for a copy.
 */
function NoteTransfer({
  note,
  workspaces,
  pending,
  intents,
}: {
  note: Note
  workspaces: ThinkingWorkspace[]
  pending: NonNullable<PendingTransfer>
  intents: NoteIntents
}) {
  const destination = transferDestination(workspaces, pending)
  if (!destination) return null
  return (
    <div className="transfer">
      <label htmlFor={`transfer-${note.id}`}>Thinking Workspace to move or copy into</label>
      <select
        autoFocus
        id={`transfer-${note.id}`}
        value={pending.targetWorkspaceId}
        onChange={(event) => intents.chooseTransferTarget(event.target.value)}
      >
        {transferDestinations(workspaces, note).map((workspace) => (
          <option key={workspace.id} value={workspace.id}>
            {workspace.name}
          </option>
        ))}
      </select>
      <p>{moveExplanation(destination, note)}</p>
      <p>{copyExplanation(destination, note)}</p>
      <div className="row">
        <button onClick={() => intents.transfer("move")}>Move Note</button>
        <button onClick={() => intents.transfer("copy")}>Copy Note</button>
        <button type="button" onClick={intents.cancelTransfer}>
          Cancel
        </button>
      </div>
    </div>
  )
}

function NoteText({
  note,
  drafts,
  intents,
}: {
  note: Note
  drafts: NoteDrafts
  intents: NoteIntents
}) {
  const draft = drafts.noteDraft
  if (draft?.id !== note.id) {
    // Markdown renders without raw HTML, so nothing in a Note executes.
    return (
      <div className="markdown">
        <ReactMarkdown remarkPlugins={[remarkGfm]}>{note.markdown}</ReactMarkdown>
      </div>
    )
  }
  return (
    <form onSubmit={intents.saveText}>
      <label htmlFor={`note-text-${note.id}`}>Note text</label>
      <textarea
        autoFocus
        id={`note-text-${note.id}`}
        rows={5}
        value={draft.markdown}
        onChange={(event) => intents.editTextDraft(event.target.value)}
      />
      <div className="row">
        <button type="submit">Save Note text</button>
        <button type="button" onClick={intents.cancelEdit}>
          Cancel
        </button>
      </div>
    </form>
  )
}

function NoteAnnotation({
  note,
  drafts,
  intents,
}: {
  note: Note
  drafts: NoteDrafts
  intents: NoteIntents
}) {
  const draft = drafts.annotationDraft
  if (draft?.id !== note.id) {
    return note.annotation ? <p className="annotation">{note.annotation}</p> : null
  }
  return (
    <form onSubmit={intents.saveAnnotation}>
      <label htmlFor={`annotation-${note.id}`}>Annotation</label>
      <textarea
        autoFocus
        id={`annotation-${note.id}`}
        rows={3}
        value={draft.text}
        placeholder="Plain-text commentary; leave empty to clear it"
        onChange={(event) => intents.editAnnotationDraft(event.target.value)}
      />
      <p className={isAnnotationTooLong(draft.text) ? "over-limit" : ""}>
        {annotationLength(draft.text)} / {MAX_ANNOTATION_SCALARS} characters
      </p>
      <div className="row">
        <button type="submit" disabled={isAnnotationTooLong(draft.text)}>
          Save Annotation
        </button>
        <button type="button" onClick={intents.cancelAnnotation}>
          Cancel
        </button>
      </div>
    </form>
  )
}

function NoteLabels({
  note,
  drafts,
  intents,
}: {
  note: Note
  drafts: NoteDrafts
  intents: NoteIntents
}) {
  const draft = drafts.labelDraft
  return (
    <div className="row" aria-label="Labels">
      {note.labels.map((label) => (
        <span className="badge" key={label.id}>{label.name} <button aria-label={`Detach ${label.name}`} onClick={() => intents.detachLabel(note, label)}>×</button> <button aria-label={`Rename ${label.name}`} onClick={() => intents.startLabelRename(label)}>Rename</button> <button aria-label={`Remove ${label.name}`} onClick={() => intents.removeLabel(label)}>Remove</button></span>
      ))}
      {draft?.noteId === note.id ? (
        <form onSubmit={intents.saveLabel}><label htmlFor={`label-${note.id}`}>Label</label><input autoFocus id={`label-${note.id}`} value={draft.name} onChange={(event) => intents.editLabelDraft(event.target.value)} /><button type="submit">Save Label</button><button type="button" onClick={intents.cancelLabel}>Cancel</button></form>
      ) : <button onClick={() => intents.startLabel(note)}>Add Label</button>}
    </div>
  )
}

function NoteRelationships({
  note,
  context,
  drafts,
  intents,
}: {
  note: Note
  context: NoteCardContext
  drafts: NoteDrafts
  intents: NoteIntents
}) {
  const draft = drafts.relateDraft
  const { notes, relationships } = context
  return (
    // Related Notes are candidates, not list items, so a Note card
    // stays the only list item a reader can land on.
    <div className="row" aria-label="Related Notes">
      {relatedNotes(notes, relationships, note.id).map((related) => (
        <span className="badge" key={related.id}>
          {notePreview(related)}
          <button
            aria-label={`Go to ${notePreview(related)}`}
            onClick={() => intents.focusNote(related.id)}
          >
            Go to Note
          </button>
          <button
            aria-label={`Remove Relationship to ${notePreview(related)}`}
            onClick={() => intents.unrelate(note, related.id)}
          >
            Remove Relationship
          </button>
        </span>
      ))}
      {draft?.noteId === note.id ? (
        <div className="relate">
          <label htmlFor={`relate-${note.id}`}>Relate to Note</label>
          <input
            autoFocus
            id={`relate-${note.id}`}
            value={draft.query}
            placeholder="Search Notes in this Thinking Workspace"
            onChange={(event) => intents.editRelateQuery(event.target.value)}
          />
          <div className="row">
            {relatableNotes(notes, relationships, note.id, draft.query).map((candidate) => (
              <button key={candidate.id} onClick={() => intents.relate(note, candidate.id)}>
                {notePreview(candidate)}
              </button>
            ))}
          </div>
          <button type="button" onClick={intents.cancelRelate}>
            Cancel
          </button>
        </div>
      ) : (
        <button onClick={() => intents.startRelate(note)}>Relate Note</button>
      )}
    </div>
  )
}

function NoteActions({
  note,
  drafts,
  intents,
}: {
  note: Note
  drafts: NoteDrafts
  intents: NoteIntents
}) {
  return (
    <div className="row">
      <label htmlFor={`note-type-${note.id}`}>Note Type</label>
      <select
        id={`note-type-${note.id}`}
        value={note.noteType}
        onChange={(event) => intents.setNoteType(note, event.target.value as NoteType)}
      >
        {NOTE_TYPES.map((noteType) => (
          <option key={noteType} value={noteType}>
            {noteTypeLabel(noteType)}
          </option>
        ))}
      </select>
      <button onClick={() => intents.startEdit(note)} disabled={drafts.noteDraft?.id === note.id}>
        Edit Note
      </button>
      <button
        onClick={() => intents.startAnnotation(note)}
        disabled={drafts.annotationDraft?.id === note.id}
      >
        {note.annotation ? "Edit Annotation" : "Add Annotation"}
      </button>
      <button aria-pressed={note.pinned} onClick={() => intents.togglePinned(note)}>
        {note.pinned ? "Unpin" : "Pin"}
      </button>
      <button onClick={() => intents.requestDelete(note)}>Delete Note</button>
    </div>
  )
}

/**
 * One Note, drawn the same way wherever it appears. The card holds no state
 * and commits nothing itself: drafts arrive as props and every change leaves
 * through the one intents object, so no view can grow its own mutation rules.
 */
export function NoteCard({
  note,
  context,
  drafts,
  intents,
  focused,
  registerElement,
}: {
  note: Note
  context: NoteCardContext
  drafts: NoteDrafts
  intents: NoteIntents
  focused: boolean
  registerElement: (element: HTMLDivElement | null) => void
}) {
  const relatedCount = degree(context.relationships, note.id)
  return (
    <div
      className={["note", note.pinned ? "pinned" : "", focused ? "focused" : ""]
        .filter(Boolean)
        .join(" ")}
      // A Note card is one self-contained piece of the thinking, whichever
      // arrangement the surrounding layout gives it. Neither view nests a
      // card directly under its group, so it carries no list semantics.
      role="article"
      aria-label={notePreview(note)}
      tabIndex={-1}
      aria-current={focused ? "true" : undefined}
      ref={registerElement}
    >
      <div className="row">
        <span className="badge">{noteTypeLabel(note.noteType)}</span>
        {note.pinned && <span className="badge">Pinned</span>}
        {relatedCount > 0 && <span className="badge">{relatedCount} related</span>}
      </div>

      <NoteText note={note} drafts={drafts} intents={intents} />
      <NoteAnnotation note={note} drafts={drafts} intents={intents} />
      <NoteLabels note={note} drafts={drafts} intents={intents} />
      <NoteRelationships note={note} context={context} drafts={drafts} intents={intents} />

      {/* Moving and copying are the only two ways a Note reaches
          another Thinking Workspace, and each says what it does. */}
      <div className="row" aria-label="Move or copy Note">
        {drafts.pendingTransfer?.noteId === note.id ? (
          <NoteTransfer
            note={note}
            workspaces={context.workspaces}
            pending={drafts.pendingTransfer}
            intents={intents}
          />
        ) : (
          <button
            disabled={transferDestinations(context.workspaces, note).length === 0}
            onClick={() => intents.startTransfer(note)}
          >
            Move or Copy Note
          </button>
        )}
      </div>

      <NoteActions note={note} drafts={drafts} intents={intents} />

      {drafts.pendingNoteDelete?.noteId === note.id && (
        <div className="confirm" role="alertdialog" aria-label="Confirm delete Note">
          <p>{noteDeleteConfirmationPrompt(drafts.pendingNoteDelete)}</p>
          <div className="row">
            <button onClick={() => intents.answerDelete("confirm")}>Delete Note</button>
            <button onClick={() => intents.answerDelete("cancel")}>Keep it</button>
          </div>
        </div>
      )}
    </div>
  )
}
