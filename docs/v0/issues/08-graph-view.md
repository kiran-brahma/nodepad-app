## Parent

Part of #1.

## What to build

Reconnect the graph view to the Thinking Graph and add consistent Relationship focus across graph, tiling, and kanban. Nodes represent Notes; links represent canonical symmetric Relationships; degree and related sets come only from Thinking Graph projections.

## Decisions

- Graph layout is derived/transient. Never persist D3 coordinates or simulation state.
- Node radius may reflect canonical degree; isolated Notes remain visible.
- Links are visually undirected and do not imply relation type.
- Hover previews related Notes. Click locks focus until click-again, Escape, view change, or deletion of the focused Note.
- Focus dims unrelated Notes consistently in every view without changing durable state.
- Selecting a graph node opens/focuses the same Note detail surface used elsewhere.
- No Synthesis node until the Synthesis slice.

## Acceptance criteria

- [ ] Every active-Workspace Note appears once and every valid Relationship appears once.
- [ ] Degree and related highlighting agree for all Notes.
- [ ] No dangling or cross-Workspace link can render.
- [ ] Hover and locked focus behavior follows the defined clearing rules.
- [ ] Tiling and kanban use the same related set for dimming.
- [ ] Note edits, deletes, and Relationship changes update the graph from committed state.
- [ ] Restart rebuilds the graph without persisted simulation coordinates.
- [ ] Empty, disconnected, dense, and single-Note graphs remain usable.

## Testing decisions

- Test public Thinking Graph projections for node/link uniqueness, degree, and related sets.
- UI integration covers cross-view focus, Escape, deletion while focused, and durable mutation refresh.
- Do not assert exact D3 coordinates, timing, or force internals.

## Blocked by

- #6
- #8

## Scope fence

Do not add typed edges, graph editing gestures, persistent coordinates, cross-Workspace graphs, AI, or new graph analytics.

## Delivery workflow

Run `prd-simplicity-audit`, use a fresh `implement` session, then `code-review`, scoped `fallow`, focused tests and repository gates, and one PR against `main`.
