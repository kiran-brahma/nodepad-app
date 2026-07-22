## Parent

Part of #1.

## What to build

Add manual Relationships and the core Thinking Graph invariants. From a Note detail surface, the thinker can search/select another Note in the active Workspace, create a Relationship, inspect related Notes, navigate to one, and remove the Relationship.

Relationships are symmetric and untyped. They represent one strong conceptual association, not direction, support, contradiction, or a named relation. Each records manual or AI provenance and creation time. This slice creates manual Relationships only but the schema must allow later AI provenance.

## Decisions

- Store each unordered Note pair once using a canonical pair ordering.
- A Relationship can connect only distinct existing Notes in the same Workspace.
- Prevent duplicate pairs at the database and module levels.
- Deleting a Note cascades every Relationship involving it.
- The relation editor searches Notes in the active Workspace and excludes the current Note and already-related Notes.
- Relationship creation defaults to manual provenance.
- Navigation/focus is transient UI state; it is not persisted as graph content.
- The Thinking Graph module owns related lookup, validation, cleanup, degree, and canonical projections.

## Acceptance criteria

- [ ] The thinker can create and remove a Relationship from the Note detail surface.
- [ ] Either endpoint lists the other as related.
- [ ] The relationship pair is stored once and duplicate creation is idempotent or returns a typed conflict without duplication.
- [ ] Self-Relationships and cross-Workspace Relationships fail without partial state.
- [ ] Deleting either Note removes the Relationship.
- [ ] Manual provenance and creation time persist after restart.
- [ ] Related-Note navigation focuses the selected durable Note without mutating the Relationship.
- [ ] Graph calculations ignore no valid relation and can never observe dangling endpoints.

## Testing decisions

- Test through Thinking Workspace intents plus public Thinking Graph projections.
- Cover reversed endpoint order, duplicates, self-link, cross-Workspace link, cascade delete, transaction rollback, reopened SQLite state, and in-memory conformance.
- UI integration covers add, inspect, navigate, and remove from a Note surface.

## Blocked by

- #4

## Scope fence

Do not add typed/directional Relationships, graph visualization, AI suggestions, cross-Workspace links, or drag-to-link interactions.

## Delivery workflow

Run `prd-simplicity-audit`, use a fresh `implement` session, then `code-review`, scoped `fallow`, focused tests and repository gates, and one PR against `main`.
