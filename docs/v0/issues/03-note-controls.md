## Parent

Part of #1.

## What to build

Complete manual Note control through the Thinking Workspace interface: edit Markdown text, delete, pin/unpin, assign the fixed Note Type, write an Annotation separately from Note text, and undo recent committed mutations during the current session.

Undo uses bounded in-memory command history and creates a new compensating SQLite transaction. It does not rewind the database file and does not survive restart. Manual Note Type and Annotation changes record manual authorship so later AI slices cannot overwrite them silently.

## Decisions

- Fixed Note Types: claim, question, task, idea, entity, quote, reference, definition, opinion, reflection, narrative, comparison, thesis, general.
- Note text is required after trimming; Markdown is stored as authored and rendered safely.
- Annotation is optional Markdown-free plain text, maximum 2,000 Unicode scalar values.
- Pinning is a boolean durable field; pinned Notes sort before unpinned Notes while retaining stable creation order within each group.
- Manual Note Type and Annotation edits set their provenance to manual.
- Maintain at most 20 reversible session commands per Workspace.
- Undo covers Note create, text edit, delete, pin, Note Type, and Annotation.
- Undo of delete restores the same Note identity and its fields.
- Restart clears undo history without changing durable state.

## Acceptance criteria

- [ ] The thinker can edit Note text and the edit survives restart.
- [ ] The thinker can delete a Note after the existing lightweight UI confirmation pattern and undo that deletion in-session.
- [ ] The thinker can pin/unpin and see deterministic ordering.
- [ ] The thinker can choose any fixed Note Type and the value survives restart.
- [ ] The thinker can add, edit, clear, and persist an Annotation independently of Note text.
- [ ] Manual provenance is durable for Note Type and Annotation.
- [ ] Undo creates committed compensating changes and covers every listed mutation.
- [ ] Undo reports an empty-history outcome without changing state.
- [ ] Restart clears undo availability but preserves the latest committed state.
- [ ] Unsafe Markdown execution is not enabled.

## Testing decisions

- Extend the highest-seam conformance suite for every mutation and compensating undo.
- Test failed edits and failed undo transactions for atomic rollback.
- Test the 20-command bound, per-Workspace isolation, restart clearing, stable identity restoration, Unicode, and validation limits.
- UI tests cover edit, type picker, Annotation, pin, delete, and keyboard undo without asserting internal React state.

## Blocked by

- #2

## Scope fence

Task remains only a Note Type. Do not add subtasks, due dates, reminders, Labels, Relationships, search, AI, or new views. Do not preserve legacy automatic task merging.

## Delivery workflow

Run `prd-simplicity-audit`, use a fresh `implement` session, then `code-review`, scoped `fallow`, focused tests and repository gates, and one PR against `main`.
