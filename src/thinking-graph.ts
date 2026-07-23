import type { Note, Relationship } from "./workspace-client"

/**
 * The Thinking Graph as the Note detail surface reads it: related lookup,
 * degree, and the candidates a new Relationship may name. Every function here
 * is a projection of committed state, so nothing in the interface has to know
 * that a symmetric pair is stored with its endpoints sorted.
 *
 * The durable rules — canonical ordering, validation, and cascade — belong to
 * the Rust Thinking Graph module. Nothing here writes.
 */

/** The endpoint that is not `noteId`, or nothing when this pair excludes it. */
function otherEndpoint(relationship: Relationship, noteId: string): string | null {
  if (relationship.noteIdA === noteId) return relationship.noteIdB
  if (relationship.noteIdB === noteId) return relationship.noteIdA
  return null
}

/** The Notes this Note is related to, in the order they were related. */
export function relatedNoteIds(relationships: Relationship[], noteId: string): string[] {
  return relationships.flatMap((relationship) => {
    const other = otherEndpoint(relationship, noteId)
    return other === null ? [] : [other]
  })
}

/** How many Notes this Note is related to. */
export function degree(relationships: Relationship[], noteId: string): number {
  return relatedNoteIds(relationships, noteId).length
}

/**
 * Related Notes as durable Notes. An endpoint with no Note is never shown,
 * though storage's cascade means one cannot arrive here.
 */
export function relatedNotes(
  notes: Note[],
  relationships: Relationship[],
  noteId: string,
): Note[] {
  const related = new Set(relatedNoteIds(relationships, noteId))
  return notes.filter((note) => related.has(note.id))
}

/**
 * What the relation editor may offer: Notes in the same Thinking Workspace,
 * never this Note, never one already related, narrowed by the thinker's text.
 */
export function relatableNotes(
  notes: Note[],
  relationships: Relationship[],
  noteId: string,
  query: string,
): Note[] {
  const note = notes.find((candidate) => candidate.id === noteId)
  if (!note) return []
  const excluded = new Set([noteId, ...relatedNoteIds(relationships, noteId)])
  const terms = query.trim().toLowerCase()
  return notes.filter(
    (candidate) =>
      candidate.workspaceId === note.workspaceId &&
      !excluded.has(candidate.id) &&
      (terms === "" ||
        `${candidate.markdown} ${candidate.annotation ?? ""}`.toLowerCase().includes(terms)),
  )
}
