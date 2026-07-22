## Parent

Part of #1.

## What to build

Reconnect the proven tiling and kanban experiences to committed Thinking Workspace projections. Both views consume the same durable Note data and submit the same intents. Switching views must never change meaning or lose state.

Tiling is an automatic spatial reading surface, not a freeform coordinate canvas. Kanban groups Notes by fixed Note Type. Search filtering, pinned ordering, Labels, Annotation, and all manual Note controls remain available where the view can present them coherently.

## Decisions

- Preserve the current BSP-inspired automatic tiling behavior where practical, but isolate layout calculation from durable domain state.
- View page, scroll, hover, selection, and minimap state are transient.
- Pinned Notes sort first before tiling; stable creation order breaks ties.
- Kanban has one visible column per Note Type represented in the current result, with deterministic type ordering.
- Search filters both views from the same active-Workspace result set.
- Views may share a Note presentation module but cannot duplicate mutation policy.
- No persistent drag ordering in V0.

## Acceptance criteria

- [ ] Tiling renders active-Workspace Notes from the committed projection and supports manual Note actions through intents.
- [ ] Kanban groups the same Notes by Note Type and supports the same applicable actions.
- [ ] Switching view preserves selection when the Note remains visible and never changes durable data.
- [ ] Restart reconstructs both views from SQLite without saved layout coordinates.
- [ ] Pinned ordering and search filtering are consistent across views.
- [ ] Labels and Annotation display without creating alternate copies of domain state.
- [ ] Empty, one-Note, many-Note, and filtered states render safely.
- [ ] Existing valuable animation and minimap behavior may be retained without making it a durable seam.

## Testing decisions

- Test projections as public pure outcomes from committed Workspace snapshots.
- UI integration covers create/edit in one view, switch, verify in the other, restart, search, empty state, and pinned order.
- Avoid snapshot tests of layout pixel coordinates or animation internals.

## Blocked by

- #4
- #5

## Scope fence

Do not add graph view, freeform placement, persistent layout, drag ordering, AI, or broad visual redesign.

## Delivery workflow

Run `prd-simplicity-audit`, use a fresh `implement` session, then `code-review`, scoped `fallow`, focused tests and repository gates, and one PR against `main`.
