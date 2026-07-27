import { useState, type FormEvent, type KeyboardEvent } from "react"
import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"
import {
  NOTE_TYPES,
  type Label,
  type Note,
  type NoteType,
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
import { nodeDegree, relatableNotes, relatedNotes, type ThinkingGraph } from "./thinking-graph"
import { ExternalLink } from "./external-link"
import { EscapeDismiss } from "./escape-dismiss"
import { useEscape, ESCAPE_PRIORITY } from "./escape-stack"
import {
  copyExplanation,
  moveExplanation,
  transferDestination,
  transferDestinations,
  type PendingTransfer,
} from "./note-transfer"
import type { NoteDrafts } from "./note-drafts"
import type { EnrichmentStatus } from "./enrichment-controller"
import { organizing as aiOrganizing } from "./ai-presence"
import type { SuggestedRelationship } from "./suggested-relationships"

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
  /** Re-runs the Enrichment Workflow against the current Note text. */
  retryEnrichment: () => void
  /** Asks for Re-enrich and Replace. The UI renders a confirmation
   *  dialog before the controller calls the Rust side with `force =
   *  true`, so a tap does not silently destroy the thinker's manual
   *  organization. */
  requestReplaceEnrichment: () => void
  /** Commits the Re-enrich and Replace after the thinker confirms. */
  confirmReplaceEnrichment: () => void
  /** Backs out of the Re-enrich and Replace dialog. */
  cancelReplaceEnrichment: () => void
  /** Dismisses a failed organization's inline affordance. Clears the status
   *  only; the Note keeps whatever the thinker last committed. */
  dismissEnrichment: () => void
  /** Commits an AI-proposed Relationship through the one relate command. */
  acceptSuggestion: (suggestion: SuggestedRelationship) => void
  /** Forgets an AI-proposed Relationship. Nothing is committed and the same
   *  pair is not offered again this session. */
  dismissSuggestion: (suggestion: SuggestedRelationship) => void
  editTextDraft: (markdown: string) => void
  editAnnotationDraft: (text: string) => void
}

/**
 * What a card reads to draw itself: the one Thinking Graph projection every
 * view shares, and the Thinking Workspaces a Note may travel to. A card
 * commits nothing from this; it only shows what already exists.
 */
export interface NoteCardContext {
  graph: ThinkingGraph
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
    // Click the thought to edit it in place.
    return (
      <div
        className="markdown"
        onClick={() => intents.startEdit(note)}
        role="button"
        tabIndex={0}
        aria-label="Edit note text"
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault()
            intents.startEdit(note)
          }
        }}
      >
        <ReactMarkdown remarkPlugins={[remarkGfm]} components={{ a: ExternalLink }}>
          {note.markdown}
        </ReactMarkdown>
      </div>
    )
  }

  function handleKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === "Escape") {
      event.preventDefault()
      intents.cancelEdit()
      return
    }
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault()
      intents.saveText({ preventDefault: () => {} } as FormEvent)
      return
    }
    // Shift+Enter inserts a newline (native textarea behavior)
  }

  return (
    <textarea
      autoFocus
      className="inline-editor"
      aria-label="Edit note text"
      value={draft.markdown}
      onChange={(event) => intents.editTextDraft(event.target.value)}
      onBlur={() => intents.saveText({ preventDefault: () => {} } as FormEvent)}
      onKeyDown={handleKeyDown}
      rows={5}
    />
  )
}

function NoteAnnotation({
  drafts,
  intents,
}: {
  drafts: NoteDrafts
  intents: NoteIntents
}) {
  // This component is only rendered when editing is active (draft matches note).
  const editingDraft = drafts.annotationDraft!

  function handleKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === "Escape") {
      event.preventDefault()
      intents.cancelAnnotation()
      return
    }
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault()
      if (!isAnnotationTooLong(editingDraft.text)) {
        intents.saveAnnotation({ preventDefault: () => {} } as FormEvent)
      }
      return
    }
    // Shift+Enter inserts a newline (native textarea behavior)
  }

  return (
    <div className="annotation-editor">
      <textarea
        autoFocus
        className="inline-editor"
        aria-label="Edit annotation"
        value={editingDraft.text}
        placeholder="Plain-text commentary; leave empty to clear it"
        onChange={(event) => intents.editAnnotationDraft(event.target.value)}
        onBlur={() => {
          if (!isAnnotationTooLong(editingDraft.text)) {
            intents.saveAnnotation({ preventDefault: () => {} } as FormEvent)
          }
        }}
        onKeyDown={handleKeyDown}
        rows={3}
      />
      <p
        className={isAnnotationTooLong(editingDraft.text) ? "over-limit" : "annotation-count"}
        role={isAnnotationTooLong(editingDraft.text) ? "alert" : undefined}
      >
        {isAnnotationTooLong(editingDraft.text)
          ? `Over the limit: ${annotationLength(editingDraft.text)} / ${MAX_ANNOTATION_SCALARS} characters`
          : `${annotationLength(editingDraft.text)} / ${MAX_ANNOTATION_SCALARS} characters`}
      </p>
    </div>
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
        <form onSubmit={intents.saveLabel}><EscapeDismiss onEscape={intents.cancelLabel} /><label htmlFor={`label-${note.id}`}>Label</label><input autoFocus id={`label-${note.id}`} value={draft.name} onChange={(event) => intents.editLabelDraft(event.target.value)} /><button type="submit">Save Label</button><button type="button" onClick={intents.cancelLabel}>Cancel</button></form>
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
  const { graph } = context
  return (
    // Related Notes are candidates, not list items, so a Note card
    // stays the only list item a reader can land on.
    <div className="row" aria-label="Related Notes">
      {relatedNotes(graph, note.id).map((related) => (
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
          <EscapeDismiss onEscape={intents.cancelRelate} />
          <label htmlFor={`relate-${note.id}`}>Relate to Note</label>
          <input
            autoFocus
            id={`relate-${note.id}`}
            value={draft.query}
            placeholder="Search Notes in this Thinking Workspace"
            onChange={(event) => intents.editRelateQuery(event.target.value)}
          />
          <div className="row">
            {relatableNotes(graph, note.id, draft.query).map((candidate) => (
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

/**
 * An AI-proposed Relationship, offered where the thought is rather than in a
 * popup: one line naming the other Note, and two small controls. Nothing is
 * committed until Link is pressed, and Dismiss commits nothing at all.
 */
function SuggestedRelationshipChips({
  context,
  intents,
  suggestions,
}: {
  context: NoteCardContext
  intents: NoteIntents
  suggestions: readonly SuggestedRelationship[]
}) {
  if (suggestions.length === 0) return null
  return (
    <div className="row suggested-relationships" aria-label="Suggested Relationships">
      {suggestions.map((suggestion) => {
        const other = context.graph.nodes.find(
          (node) => node.note.id === suggestion.otherNoteId,
        )?.note
        if (!other) return null
        const preview = notePreview(other)
        return (
          <span className="suggested-relationship" key={suggestion.key}>
            <span className="tag tag-neutral">Relate to “{preview}”?</span>
            <button
              aria-label={`Link to ${preview}`}
              className="tag-outline"
              onClick={() => intents.acceptSuggestion(suggestion)}
              type="button"
            >
              Link
            </button>
            <button
              aria-label={`Dismiss suggested Relationship to ${preview}`}
              className="tag-neutral"
              onClick={() => intents.dismissSuggestion(suggestion)}
              type="button"
            >
              Dismiss
            </button>
          </span>
        )
      })}
    </div>
  )
}

/**
 * The quiet per-Note piece of the Enrichment Workflow. The same four
 * states as before — organizing, applied, failed, replace-pending — drawn
 * as unobtrusive affordances rather than attention-seeking ones: while AI
 * organizes, the card shimmers and this line carries only the status text
 * a screen reader needs. It never blocks a thinker from editing a Note.
 */
function NoteEnrichmentBadge({
  note,
  status,
  onRetry,
  onReplace,
  onConfirmReplace,
  onCancelReplace,
  onDismiss,
}: {
  note: Note
  status?: EnrichmentStatus
  onRetry: () => void
  onReplace: () => void
  onConfirmReplace: () => void
  onCancelReplace: () => void
  onDismiss: () => void
}) {
  if (status?.kind === "replace_pending") {
    return (
      <span
        className="row"
        role="alertdialog"
        aria-label="Confirm Re-enrich and Replace"
      >
        <EscapeDismiss onEscape={onCancelReplace} />
        <span>{status.reason}</span>
        <button onClick={onConfirmReplace}>Replace</button>
        <button onClick={onCancelReplace}>Keep manual</button>
      </span>
    )
  }
  if (!status || status.kind === "idle" || status.kind === "cancelled") {
    if (note.lastEnrichedAt) {
      return <span className="badge quiet" aria-label="Organized by AI">AI organized</span>
    }
    return null
  }
  if (aiOrganizing(status)) {
    // The shimmer on the card is the visible signal; this stays a plain,
    // quiet line so nothing competes with the thought itself. It is not a
    // live region: the card's `aria-busy` and the one top-bar indicator
    // announce the same fact, and announcing it three times is not quiet.
    return <span className="enrich-organizing">Organizing…</span>
  }
  if (status.kind === "applied") {
    return <span className="badge quiet" aria-label="Organized by AI">AI organized</span>
  }
  return (
    <NoteEnrichmentFailureBadge
      status={status}
      onRetry={onRetry}
      onReplace={onReplace}
      onDismiss={onDismiss}
    />
  )
}

/** A failed organization is recoverable inline: retry, replace when the
 *  response was overtaken by an edit, or dismiss. No modal, and dismissing
 *  leaves the Note exactly as the thinker left it. */
function NoteEnrichmentFailureBadge({
  status,
  onRetry,
  onReplace,
  onDismiss,
}: {
  status: Extract<EnrichmentStatus, { kind: "failed" }>
  onRetry: () => void
  onReplace: () => void
  onDismiss: () => void
}) {
  const label = failureBadgeLabel(status.reason)
  return (
    <span className="row enrich-failed" role="status" aria-label="AI assistance status" aria-live="polite">
      <span className="badge quiet">{label}</span>
      <button onClick={onRetry}>Retry</button>
      {status.reason === "stale" && <button onClick={onReplace}>Re-enrich and Replace</button>}
      <button aria-label="Dismiss AI assistance status" onClick={onDismiss}>
        Dismiss
      </button>
    </span>
  )
}

function failureBadgeLabel(reason: "stale" | "invalid_schema" | "provider" | "unavailable"): string {
  switch (reason) {
    case "stale":
      return "Try again"
    case "invalid_schema":
      return "AI returned bad data"
    case "provider":
      return "AI request failed"
    case "unavailable":
      return "AI unavailable"
  }
}

/**
 * The hover action row: compact icon buttons at the card's top-right.
 * Revealed on card hover and reachable via the ⌘· menu.
 */
function ActionRow({
  note,
  intents,
  onOpenMenu,
}: {
  note: Note
  intents: NoteIntents
  onOpenMenu: () => void
}) {
  return (
    <div className="action-row" aria-label="Note actions">
      <button
        className="action-btn"
        onClick={() => intents.startRelate(note)}
        aria-label="Relate"
        title="Relate"
      >
        🔗
      </button>
      <button
        className="action-btn"
        onClick={() => intents.togglePinned(note)}
        aria-label={note.pinned ? "Unpin" : "Pin"}
        title={note.pinned ? "Unpin" : "Pin"}
      >
        {note.pinned ? "📌" : "📍"}
      </button>
      <button
        className="action-btn"
        onClick={onOpenMenu}
        aria-label="More actions"
        title="More actions (⌘·)"
      >
        ⋯
      </button>
    </div>
  )
}

/**
 * The ⌘· menu: exposes the full action set. Opened by clicking "⋯" in the
 * action row or pressing ⌘· with the card focused.
 */
function CommandMenu({
  note,
  intents,
  onClose,
}: {
  note: Note
  intents: NoteIntents
  onClose: () => void
}) {
  useEscape(onClose, ESCAPE_PRIORITY.dialog)

  return (
    <div className="command-menu" role="menu" aria-label="Note command menu">
      <EscapeDismiss onEscape={onClose} />
      <button
        role="menuitem"
        onClick={() => { intents.togglePinned(note); onClose() }}
      >
        {note.pinned ? "Unpin" : "Pin"}
      </button>

      <button
        role="menuitem"
        onClick={() => { intents.startLabel(note); onClose() }}
      >
        Add Label
      </button>
      <button
        role="menuitem"
        onClick={() => { intents.startRelate(note); onClose() }}
      >
        Relate
      </button>
      <button
        role="menuitem"
        onClick={() => { intents.startTransfer(note); onClose() }}
      >
        Move/Copy
      </button>
      <button
        role="menuitem"
        onClick={() => { intents.requestDelete(note); onClose() }}
      >
        Delete
      </button>
    </div>
  )
}

/**
 * One Note, drawn the same way wherever it appears. The card holds no state
 * and commits nothing itself: drafts arrive as props and every change leaves
 * through the one intents object, so no view can grow its own mutation rules.
 *
 * At rest the card shows only the Note Type tag, the rendered thought, a
 * quiet Annotation aside, and a small meta line. Secondary actions move
 * behind a hover action row and a ⌘· menu, both reachable by keyboard.
 */
export function NoteCard({
  note,
  context,
  drafts,
  intents,
  focused,
  dimmed,
  registerElement,
  enrichment,
  suggestions = [],
}: {
  note: Note
  context: NoteCardContext
  drafts: NoteDrafts
  intents: NoteIntents
  focused: boolean
  /** Focus elsewhere leaves this Note unrelated to it. Dimming commits nothing. */
  dimmed: boolean
  registerElement: (element: HTMLDivElement | null) => void
  /** The Enrichment Workflow status for this Note, or `undefined` if
   *  the active Workspace's policy does not permit AI assistance. */
  enrichment?: EnrichmentStatus
  /** The AI-proposed Relationships this Note's organization raised and the
   *  thinker has not answered. Never a committed Relationship. */
  suggestions?: readonly SuggestedRelationship[]
}) {
  const [commandMenuOpen, setCommandMenuOpen] = useState(false)
  const [typePopoverOpen, setTypePopoverOpen] = useState(false)

  // The badge counts the same links the graph draws and the chips below list,
  // because all three read one projection.
  const relatedCount = nodeDegree(context.graph, note.id)
  const firstLabel = note.labels[0]?.name
  // Whether AI is organizing this Note right now, asked once and answered by
  // the one presence derivation, so the shimmer, `aria-busy`, and the meta
  // line cannot disagree.
  const organizing = aiOrganizing(enrichment)

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    // ⌘· (Command+Period) opens or closes the command menu
    if (event.metaKey && event.key === ".") {
      event.preventDefault()
      event.stopPropagation()
      setCommandMenuOpen((prev) => !prev)
    }
  }

  function closeMenu() {
    setCommandMenuOpen(false)
  }

  return (
    <div
      className={[
        "note",
        note.pinned ? "pinned" : "",
        focused ? "focused" : "",
        dimmed ? "dimmed" : "",
        // A shimmer while AI organizes this Note. Presentation only: the card
        // stays fully editable, and the durable revision guard is what
        // invalidates a response the thinker's edit has overtaken.
        organizing ? "organizing" : "",
      ]
        .filter(Boolean)
        .join(" ")}
      // A Note card is one self-contained piece of the thinking, whichever
      // arrangement the surrounding layout gives it. Neither view nests a
      // card directly under its group, so it carries no list semantics.
      role="article"
      aria-label={notePreview(note)}
      tabIndex={-1}
      aria-current={focused ? "true" : undefined}
      // Something is being worked out about this Note. It stays editable; the
      // flag only tells assistive technology the card may change under it.
      aria-busy={organizing || undefined}
      ref={registerElement}
      onKeyDown={handleKeyDown}
    >
      {/* ── Resting layout ────────────────────────────────────────────── */}
      <div className="note-top">
        <div className="note-top-left">
          <span
            className="tag type-tag"
            onClick={() => setTypePopoverOpen((prev) => !prev)}
            role="button"
            tabIndex={0}
            aria-label="Change Note Type"
            aria-haspopup="listbox"
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault()
                setTypePopoverOpen((prev) => !prev)
              }
            }}
          >
            {noteTypeLabel(note.noteType)}
          </span>
          {note.noteTypeProvenance === "ai" && (
            <span className="badge" aria-label="Note Type organized by AI">AI</span>
          )}
          {typePopoverOpen && (
            <div
              className="type-popover"
              role="listbox"
              aria-label="Note Types"
              onClick={(event) => event.stopPropagation()}
            >
              {NOTE_TYPES.map((noteType) => (
                <button
                  key={noteType}
                  className={noteType === note.noteType ? "type-option selected" : "type-option"}
                  role="option"
                  aria-selected={noteType === note.noteType}
                  onClick={() => {
                    intents.setNoteType(note, noteType)
                    setTypePopoverOpen(false)
                  }}
                >
                  {noteTypeLabel(noteType)}
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Hover action row — visible on card hover */}
        <ActionRow note={note} intents={intents} onOpenMenu={() => setCommandMenuOpen(true)} />

        {/* ⌘· menu — opened by clicking ⋯ or pressing ⌘· */}
        {commandMenuOpen && (
          <CommandMenu note={note} intents={intents} onClose={closeMenu} />
        )}
      </div>

      {/* Rendered thought */}
      <NoteText note={note} drafts={drafts} intents={intents} />

      {/* Annotation as quiet left-ruled aside (only when not editing) */}
      {drafts.annotationDraft?.id !== note.id && (
        <>
          {note.annotation ? (
            <div className="annotation-aside">
              <p
                onClick={() => intents.startAnnotation(note)}
                role="button"
                tabIndex={0}
                aria-label="Edit annotation"
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault()
                    intents.startAnnotation(note)
                  }
                }}
              >
                {note.annotation}
              </p>
              {note.annotationProvenance === "ai" && (
                <span className="badge" aria-label="Annotation organized by AI">AI</span>
              )}
            </div>
          ) : (
            <div className="annotation-aside annotation-add">
              <span
                onClick={() => intents.startAnnotation(note)}
                role="button"
                tabIndex={0}
                aria-label="Add annotation"
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault()
                    intents.startAnnotation(note)
                  }
                }}
              >
                Add annotation
              </span>
            </div>
          )}
        </>
      )}

      {/* Meta line: linked count, first label, enrichment status */}
      <div className="meta">
        {relatedCount > 0 && <span>{relatedCount} linked</span>}
        {relatedCount > 0 && firstLabel && <span className="meta-sep">·</span>}
        {firstLabel && <span>{firstLabel}</span>}
        <NoteEnrichmentBadge
          note={note}
          status={enrichment}
          onRetry={intents.retryEnrichment}
          onReplace={intents.requestReplaceEnrichment}
          onConfirmReplace={intents.confirmReplaceEnrichment}
          onCancelReplace={intents.cancelReplaceEnrichment}
          onDismiss={intents.dismissEnrichment}
        />
      </div>

      {/* Undecided AI proposals: a quiet chip, never a modal */}
      <SuggestedRelationshipChips
        context={context}
        intents={intents}
        suggestions={suggestions}
      />

      {/* ── Draft editors (shown when active) ─────────────────────────── */}

      {/* Annotation editor */}
      {drafts.annotationDraft?.id === note.id && (
        <NoteAnnotation drafts={drafts} intents={intents} />
      )}

      {/* Label editor */}
      {drafts.labelDraft?.noteId === note.id && (
        <NoteLabels note={note} drafts={drafts} intents={intents} />
      )}

      {/* Relate editor */}
      {drafts.relateDraft?.noteId === note.id && (
        <NoteRelationships note={note} context={context} drafts={drafts} intents={intents} />
      )}

      {/* Transfer editor */}
      {drafts.pendingTransfer?.noteId === note.id && (
        <div className="row" aria-label="Move or copy Note">
          <NoteTransfer
            note={note}
            workspaces={context.workspaces}
            pending={drafts.pendingTransfer}
            intents={intents}
          />
        </div>
      )}

      {/* Delete confirmation */}
      {drafts.pendingNoteDelete?.noteId === note.id && (
        <div className="confirm" role="alertdialog" aria-label="Confirm delete Note">
          <EscapeDismiss onEscape={() => intents.answerDelete("cancel")} />
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
