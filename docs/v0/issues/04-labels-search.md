## Parent

Part of #1.

## What to build

Add free-form Workspace Labels and active-Workspace search. The thinker can create, attach, detach, rename, and remove Labels from Notes. Search matches Note text, Annotation, and Labels through SQLite full-text search and never searches another Workspace in V0.

## Decisions

- Labels are scoped to one Thinking Workspace.
- A Label display name is trimmed, one to four words, maximum 60 Unicode scalar values.
- Identity comparison uses Unicode-aware case folding where the chosen SQLite/tooling stack supports it; otherwise normalize consistently in the Thinking Workspace module and document the limitation.
- Preserve one display spelling while preventing case-only duplicate Labels in a Workspace.
- Removing the final membership may delete the unused Label.
- Renaming to an existing normalized Label merges memberships atomically and removes the duplicate identity.
- Search is active-Workspace-only. No global search or automatic Workspace switching.
- Search results contain Note identity, a safe text snippet, Note Type, matched Labels, and rank.
- Empty search restores the normal view; search itself never mutates durable state.

## Acceptance criteria

- [ ] The thinker can create and attach multiple Labels to a Note.
- [ ] Case-only and whitespace variants reuse existing Label identity.
- [ ] The thinker can detach and remove Labels without changing Note text.
- [ ] Rename persists across restart and merge-on-collision preserves all memberships.
- [ ] Search matches Note text, Annotation, and Label names inside the active Workspace.
- [ ] Results never include another Workspace and identify Note Type and matched Labels.
- [ ] Search handles Unicode and special FTS query characters without errors or injection.
- [ ] Label and search behavior uses the Thinking Workspace interface or read projection, not direct UI SQL.

## Testing decisions

- Extend adapter conformance for Label lifecycle, membership uniqueness, rename merge, and transaction rollback.
- Run SQLite FTS tests against a reopened temporary database.
- Test cross-Workspace isolation, Unicode, empty search, punctuation, case variants, deleted Notes, and deterministic ranking ties.
- Add one UI path for labeling and searching without coupling to private view structure.

## Blocked by

- #4

## Scope fence

Do not add global search, saved searches, Label hierarchies, colors, AI suggestions, Relationships, or new views.

## Delivery workflow

Run `prd-simplicity-audit`, use a fresh `implement` session, then `code-review`, scoped `fallow`, focused tests and repository gates, and one PR against `main`.
