## Parent

Part of #1.

## What to build

Add atomic move and copy operations between Thinking Workspaces. Moving preserves Note identity and authored fields but removes every Relationship because V0 Relationships cannot cross Workspace seams. Copying preserves the Note's text, Note Type, Annotation, Labels by display meaning, pin state, and manual provenance, but assigns fresh Note identity, timestamp, and no Relationships.

## Decisions

- Source and target must be distinct existing Workspaces.
- Move uses one transaction covering Workspace reassignment, Label membership remapping, and Relationship cleanup.
- Labels are mapped by normalized display name into the target Workspace, creating them only when missing.
- Copy uses one transaction and fresh collision-resistant identity.
- Copy retains Label meanings through target Label identities; it does not share Label rows across Workspaces.
- No Relationship is copied or moved.
- Undo move returns the Note to its prior Workspace and restores only Relationships captured by the command when both endpoints still exist in that prior Workspace.
- Undo copy deletes only the created copy.

## Acceptance criteria

- [ ] The thinker can move a Note to another Workspace and it disappears from the source and appears in the target after commit.
- [ ] Move preserves identity and authored fields, remaps Labels, and removes invalid Relationships atomically.
- [ ] The thinker can copy a Note and both source and fresh target identity remain.
- [ ] Copy preserves intended content/organization but inherits no Relationship.
- [ ] Target Label case variants are reused without duplication.
- [ ] Failed move/copy leaves both Workspaces unchanged.
- [ ] Move/copy outcomes survive restart.
- [ ] Undo follows the defined restoration rules without creating dangling Relationships.

## Testing decisions

- Extend adapter conformance with move/copy success, failure injection, Label remap, identity collision, Relationship cleanup, and undo.
- Reopen SQLite for durability assertions.
- UI path exposes destination choice and clearly distinguishes move from copy.

## Blocked by

- #3
- #6

## Scope fence

Do not support cross-Workspace Relationships, bulk move/copy, drag-and-drop between Workspaces, or merge Workspaces.

## Delivery workflow

Run `prd-simplicity-audit`, use a fresh `implement` session, then `code-review`, scoped `fallow`, focused tests and repository gates, and one PR against `main`.
