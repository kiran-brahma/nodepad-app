import { useMemo, type KeyboardEvent, type ReactNode } from "react"
import type { Note, PendingSynthesis } from "./workspace-client"
import { notePreview } from "./note-controls"
import type { ThinkingGraph } from "./thinking-graph"
import { graphLayout, GRAPH_HEIGHT, GRAPH_WIDTH, type GraphPlacement } from "./graph-layout"
import type { NoteFocus } from "./note-focus"
import { synthesisAnchors, type SynthesisAnchor } from "./synthesis-placement"

/** Enough of a Note to recognise its node without redrawing the Note on it. */
function nodeLabel(note: Note): string {
  const preview = notePreview(note)
  return [...preview].length > 24 ? `${[...preview].slice(0, 24).join("")}…` : preview
}

function GraphNodeMark({
  placement,
  focus,
}: {
  placement: GraphPlacement
  focus: NoteFocus
}) {
  const { note } = placement.node
  const locked = focus.focusedNoteId === note.id
  const dimmed = focus.litNoteIds !== null && !focus.litNoteIds.has(note.id)

  function toggleOnKey(event: KeyboardEvent<SVGCircleElement>) {
    if (event.key !== "Enter" && event.key !== " ") return
    event.preventDefault()
    focus.toggleFocus(note.id)
  }

  return (
    <g className={dimmed ? "graph-node dimmed" : "graph-node"}>
      <circle
        // A node is the Note, so it is reached and pressed like anything else
        // that opens a Note, and its pressed state says whether focus is locked.
        role="button"
        tabIndex={0}
        aria-label={notePreview(note)}
        aria-pressed={locked}
        className={locked ? "graph-mark locked" : "graph-mark"}
        cx={placement.x}
        cy={placement.y}
        r={placement.radius}
        onClick={() => focus.toggleFocus(note.id)}
        onKeyDown={toggleOnKey}
        onMouseEnter={() => focus.hoverNote(note.id)}
        onMouseLeave={() => focus.hoverNote(null)}
        onFocus={() => focus.hoverNote(note.id)}
        onBlur={() => focus.hoverNote(null)}
      >
        <title>{notePreview(note)}</title>
      </circle>
      <text
        aria-hidden="true"
        className="graph-label"
        x={placement.x}
        y={placement.y + placement.radius + 13}
        textAnchor="middle"
      >
        {nodeLabel(note)}
      </text>
    </g>
  )
}

/** A provisional mark is drawn smaller than any Note, so it never reads as
 *  a thought the Thinking Workspace already holds. */
const SYNTHESIS_RADIUS = 8

/**
 * Where each pending Synthesis sits on the graph: the shared placement rule,
 * read against the same layout the Notes are drawn from. Nothing is stored,
 * so dismissing a Synthesis simply stops drawing it.
 */
function provisionalMarks(
  pending: PendingSynthesis[],
  placements: GraphPlacement[],
): SynthesisAnchor[] {
  return synthesisAnchors(pending, (noteId) => {
    const placement = placements.find((candidate) => candidate.node.note.id === noteId)
    return placement ? { x: placement.x, y: placement.y } : null
  })
}

/**
 * The Thinking Graph drawn: one node per Note of the active Thinking
 * Workspace, one line per Relationship. Lines are undirected and carry no
 * relation type, because a Relationship has none. Hovering a node previews
 * what it is related to and clicking one locks that focus; both dim the rest
 * of the graph and change nothing durable.
 *
 * Selecting a node opens the same Note card every other view places, over the
 * same intents, so the graph can offer no action the other views lack.
 */
export function GraphView({
  graph,
  focus,
  card,
  pendingSyntheses,
}: {
  graph: ThinkingGraph
  focus: NoteFocus
  card: (note: Note) => ReactNode
  /**
   * The undecided Syntheses of this Workspace. Each is drawn as a
   * provisional mark sitting among the Notes it claims to rest on, with
   * dashed leaders to them. Nothing about it is durable: it is not a Note in
   * the Thinking Graph, and no Relationship is committed for a Synthesis
   * that may still be dismissed.
   */
  pendingSyntheses: PendingSynthesis[]
}) {
  const layout = useMemo(() => graphLayout(graph), [graph])
  const provisional = useMemo(
    () => provisionalMarks(pendingSyntheses, layout.placements),
    [pendingSyntheses, layout.placements],
  )
  const focusedNote = graph.nodes.find((node) => node.note.id === focus.focusedNoteId)?.note

  if (layout.placements.length === 0) return <p>No Notes yet.</p>

  return (
    <div className="graph">
      <svg
        className="graph-canvas"
        role="group"
        aria-label="Thinking Graph"
        viewBox={`0 0 ${GRAPH_WIDTH} ${GRAPH_HEIGHT}`}
      >
        <g aria-hidden="true">
          {layout.links.map((link) => {
            const lit =
              focus.litNoteIds === null ||
              (focus.litNoteIds.has(link.source.node.note.id) &&
                focus.litNoteIds.has(link.target.node.note.id))
            return (
              <line
                className={lit ? "graph-link" : "graph-link dimmed"}
                key={link.id}
                x1={link.source.x}
                y1={link.source.y}
                x2={link.target.x}
                y2={link.target.y}
              />
            )
          })}
        </g>
        {layout.placements.map((placement) => (
          <GraphNodeMark key={placement.node.note.id} placement={placement} focus={focus} />
        ))}
        {provisional.map((mark) => (
          <g className="graph-synthesis" key={mark.id}>
            {mark.leaders.map((leader) => (
              <line
                aria-hidden="true"
                className="graph-synthesis-leader"
                key={`${mark.id}-${leader.noteId}`}
                strokeDasharray="4 4"
                x1={mark.x}
                y1={mark.y}
                x2={leader.x}
                y2={leader.y}
              />
            ))}
            <circle
              aria-label={`Pending Synthesis: ${mark.text}`}
              className="graph-synthesis-mark"
              cx={mark.x}
              cy={mark.y}
              r={SYNTHESIS_RADIUS}
              role="img"
              strokeDasharray="3 3"
            >
              <title>{mark.text}</title>
            </circle>
          </g>
        ))}
      </svg>

      {focusedNote && <div className="graph-detail">{card(focusedNote)}</div>}
    </div>
  )
}
