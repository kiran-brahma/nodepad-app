import type { PendingSynthesis } from "./workspace-client"

export interface Anchor {
  x: number
  y: number
}

/**
 * A pending Synthesis placed among the Notes it names: the centroid of the
 * sources that are actually drawn, plus a leader to each of them.
 *
 * It is not a Note and holds no position. The arrangement is derived on every
 * render from wherever the view already put its Notes, so accepting one is the
 * only thing that ever commits anything, and dismissing one simply stops it
 * from being drawn.
 */
export interface SynthesisAnchor {
  id: string
  text: string
  stale: boolean
  /** How many Notes the Synthesis rests on, which is what it claims to
   *  connect — not how many of them this view happens to be drawing. */
  sourceCount: number
  x: number
  y: number
  leaders: { noteId: string; x: number; y: number }[]
}

/**
 * The one answer to where a pending Synthesis belongs. Every view passes the
 * anchor it draws a Note at — a graph placement, a canvas card centre — and
 * gets the same centroid and the same rule back: a Synthesis none of whose
 * sources are drawn has nothing to sit among, so it is not drawn either.
 */
export function synthesisAnchors(
  pending: readonly PendingSynthesis[],
  anchorOf: (noteId: string) => Anchor | null,
): SynthesisAnchor[] {
  return pending.flatMap((synthesis) => {
    const leaders = synthesis.sourceNoteIds.flatMap((noteId) => {
      const anchor = anchorOf(noteId)
      return anchor ? [{ noteId, x: anchor.x, y: anchor.y }] : []
    })
    if (leaders.length === 0) return []
    return [
      {
        id: synthesis.id,
        text: synthesis.text,
        stale: synthesis.stale,
        sourceCount: synthesis.sourceNoteIds.length,
        x: leaders.reduce((total, leader) => total + leader.x, 0) / leaders.length,
        y: leaders.reduce((total, leader) => total + leader.y, 0) / leaders.length,
        leaders,
      },
    ]
  })
}
