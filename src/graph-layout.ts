import {
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  forceX,
  forceY,
  type SimulationLinkDatum,
  type SimulationNodeDatum,
} from "d3"
import type { GraphNode, ThinkingGraph } from "./thinking-graph"

/**
 * Where the Thinking Graph's nodes sit when it is drawn. The arrangement is
 * derived, not stored: a force simulation is run to rest inside this call and
 * only its result leaves, so there is no simulation to own, no coordinate to
 * commit, and a restart rebuilds the same picture from SQLite alone.
 */

/** The canvas the graph is drawn into, in its own coordinates. */
export const GRAPH_WIDTH = 720
export const GRAPH_HEIGHT = 480

/** An unrelated Note is a smaller node, never an invisible one. */
export const MIN_NODE_RADIUS = 11
const MAX_NODE_RADIUS = 24

/** Enough passes for the arrangement to settle; it is run once, not animated. */
const SETTLING_PASSES = 260

export interface GraphPlacement {
  node: GraphNode
  x: number
  y: number
  radius: number
}

export interface PlacedGraphLink {
  id: string
  source: GraphPlacement
  target: GraphPlacement
}

export interface GraphLayout {
  placements: GraphPlacement[]
  links: PlacedGraphLink[]
}

/**
 * How large a node is drawn. Radius grows with degree against the most related
 * Note in this graph, so the picture says which thoughts are connected without
 * claiming a number. A graph with no Relationship at all is every node at the
 * floor, which is a disconnected Workspace read correctly, not a special case.
 */
export function nodeRadius(degree: number, maxDegree: number): number {
  if (maxDegree === 0) return MIN_NODE_RADIUS
  const share = Math.sqrt(Math.min(degree, maxDegree) / maxDegree)
  return MIN_NODE_RADIUS + (MAX_NODE_RADIUS - MIN_NODE_RADIUS) * share
}

interface SimulationNode extends SimulationNodeDatum {
  id: string
  radius: number
}

/** Keeps a node whole inside the canvas whatever the simulation settled on. */
function clamp(value: number, radius: number, extent: number): number {
  return Math.min(Math.max(value, radius), extent - radius)
}

/**
 * The golden angle spread the simulation starts from. Starting every node at a
 * distinct, computed point makes the arrangement a function of the graph alone:
 * the same committed state lays out the same way every time it is drawn.
 */
function startingPoint(index: number): { x: number; y: number } {
  const angle = index * Math.PI * (3 - Math.sqrt(5))
  const distance = 12 * Math.sqrt(index)
  return {
    x: GRAPH_WIDTH / 2 + distance * Math.cos(angle),
    y: GRAPH_HEIGHT / 2 + distance * Math.sin(angle),
  }
}

/** The graph arranged for drawing. Nothing here is stored or committed. */
export function graphLayout(graph: ThinkingGraph): GraphLayout {
  if (graph.nodes.length === 0) return { placements: [], links: [] }

  const maxDegree = graph.nodes.reduce((most, node) => Math.max(most, node.degree), 0)
  const simulationNodes: SimulationNode[] = graph.nodes.map((node, index) => ({
    id: node.note.id,
    radius: nodeRadius(node.degree, maxDegree),
    ...startingPoint(index),
  }))
  const simulationLinks: SimulationLinkDatum<SimulationNode>[] = graph.links.map((link) => ({
    source: link.noteIdA,
    target: link.noteIdB,
  }))

  const simulation = forceSimulation(simulationNodes)
    .force(
      "link",
      forceLink<SimulationNode, SimulationLinkDatum<SimulationNode>>(simulationLinks)
        .id((node) => node.id)
        .distance(110)
        .strength(0.4),
    )
    .force("charge", forceManyBody<SimulationNode>().strength(-320))
    .force(
      "collide",
      forceCollide<SimulationNode>().radius((node) => node.radius + 14),
    )
    .force("centreX", forceX<SimulationNode>(GRAPH_WIDTH / 2).strength(0.07))
    .force("centreY", forceY<SimulationNode>(GRAPH_HEIGHT / 2).strength(0.07))
    // Stopped before the first pass, so no animation frame is ever scheduled.
    .stop()

  simulation.tick(SETTLING_PASSES)

  const placements = graph.nodes.map((node, index) => {
    const settled = simulationNodes[index]
    return {
      node,
      radius: settled.radius,
      x: clamp(settled.x ?? GRAPH_WIDTH / 2, settled.radius, GRAPH_WIDTH),
      y: clamp(settled.y ?? GRAPH_HEIGHT / 2, settled.radius, GRAPH_HEIGHT),
    }
  })

  const byNoteId = new Map(placements.map((placement) => [placement.node.note.id, placement]))
  const links = graph.links.flatMap((link) => {
    const source = byNoteId.get(link.noteIdA)
    const target = byNoteId.get(link.noteIdB)
    // The projection admits no link without both endpoints, so this only
    // states that a line is drawn between two placed Notes or not at all.
    return source && target ? [{ id: link.id, source, target }] : []
  })

  return { placements, links }
}
