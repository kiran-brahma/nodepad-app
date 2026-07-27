import { useCallback, useMemo, useState } from "react"
import type { ThinkingGraph } from "./thinking-graph"

/**
 * A Relationship the AI proposed and the thinker has not answered. It is not
 * a Relationship: nothing durable holds it, the Thinking Graph does not know
 * it, and it disappears with the session. Only accepting one commits a link.
 *
 * `noteId` is the Note whose organization proposed it, so exactly one chip is
 * drawn for a pair; `key` is the canonical pair identity, which is what
 * dedupe and dismissal are asked about.
 */
export interface SuggestedRelationship {
  noteId: string
  otherNoteId: string
  key: string
}

/** One identity per pair, whichever side proposed it. Ordering is storage,
 *  never direction — the same rule the Thinking Graph applies to links. */
export function suggestionKey(noteId: string, otherNoteId: string): string {
  return noteId <= otherNoteId ? `${noteId} ${otherNoteId}` : `${otherNoteId} ${noteId}`
}

function suggestion(noteId: string, otherNoteId: string): SuggestedRelationship {
  return { noteId, otherNoteId, key: suggestionKey(noteId, otherNoteId) }
}

/**
 * What is still worth showing. Everything the Thinking Graph already answers
 * is subtracted here rather than tracked: a pair whose endpoints are not both
 * displayed, a pair that is already related — which is what an accepted
 * suggestion becomes — and a pair the thinker dismissed this session.
 */
export function visibleSuggestions(
  graph: ThinkingGraph,
  proposed: readonly SuggestedRelationship[],
  dismissed: ReadonlySet<string>,
): SuggestedRelationship[] {
  const present = new Set(graph.nodes.map((node) => node.note.id))
  const related = new Set(graph.links.map((link) => suggestionKey(link.noteIdA, link.noteIdB)))
  return proposed.filter(
    (candidate) =>
      candidate.noteId !== candidate.otherNoteId &&
      present.has(candidate.noteId) &&
      present.has(candidate.otherNoteId) &&
      !related.has(candidate.key) &&
      !dismissed.has(candidate.key),
  )
}

export interface SuggestedRelationships {
  /** Every suggestion still worth answering, in the order it was proposed. */
  visible: SuggestedRelationship[]
  /** The suggestions whose chip belongs on this Note. */
  forNote: (noteId: string) => SuggestedRelationship[]
  /** Records what one organization result proposed for one Note. */
  propose: (noteId: string, otherNoteIds: readonly string[]) => void
  /** Commits the link through the one relate command and clears the chip. */
  accept: (suggestion: SuggestedRelationship) => void
  /** Forgets the suggestion for this session; nothing is committed. */
  dismiss: (suggestion: SuggestedRelationship) => void
}

/**
 * The session's suggested Relationships. The hook holds only what the
 * Thinking Graph cannot answer — which pairs were proposed, and which the
 * thinker waved off — and derives everything else from the graph, so the
 * application never has a second answer to what is linked.
 */
export function useSuggestedRelationships(
  graph: ThinkingGraph,
  relate: (noteId: string, otherNoteId: string) => void,
): SuggestedRelationships {
  const [proposed, setProposed] = useState<SuggestedRelationship[]>([])
  const [dismissed, setDismissed] = useState<ReadonlySet<string>>(() => new Set<string>())

  const propose = useCallback((noteId: string, otherNoteIds: readonly string[]) => {
    setProposed((current) => {
      const seen = new Set(current.map((candidate) => candidate.key))
      const added = otherNoteIds
        .map((otherNoteId) => suggestion(noteId, otherNoteId))
        .filter((candidate) => {
          if (candidate.noteId === candidate.otherNoteId) return false
          if (seen.has(candidate.key)) return false
          seen.add(candidate.key)
          return true
        })
      return added.length === 0 ? current : [...current, ...added]
    })
  }, [])

  const forget = useCallback((key: string) => {
    setProposed((current) => current.filter((candidate) => candidate.key !== key))
  }, [])

  const accept = useCallback(
    (accepted: SuggestedRelationship) => {
      // The link is committed by the one relate command, so an accepted
      // suggestion is an ordinary Relationship from the next snapshot on.
      relate(accepted.noteId, accepted.otherNoteId)
      forget(accepted.key)
    },
    [forget, relate],
  )

  const dismiss = useCallback(
    (rejected: SuggestedRelationship) => {
      // Dismissal commits nothing. The key is remembered only so the same
      // unchanged pair is not offered again this session.
      setDismissed((current) => new Set(current).add(rejected.key))
      forget(rejected.key)
    },
    [forget],
  )

  const visible = useMemo(
    () => visibleSuggestions(graph, proposed, dismissed),
    [graph, proposed, dismissed],
  )

  const forNote = useCallback(
    (noteId: string) => visible.filter((candidate) => candidate.noteId === noteId),
    [visible],
  )

  return { visible, forNote, propose, accept, dismiss }
}
