## Parent

Part of #1.

## What to build

Complete Thinking Workspace lifecycle on the durable foundation: create, list, select, rename, and confirm-delete Workspaces; enforce the invariant that the application always has one valid Workspace; and show a safe recovery state when the database cannot be opened.

Every lifecycle action is an explicit Thinking Workspace intent and a single SQLite transaction. Selection is a durable non-secret preference so the last active Workspace reopens when it still exists. Deleting the active Workspace selects a deterministic survivor. The UI must never silently replace corrupt or unreadable data with a blank database.

## Decisions

- Workspace names are required after trimming and limited to 120 Unicode scalar values.
- Duplicate names are allowed; identity is never derived from the name.
- Delete always requires an explicit confirmation naming the Workspace.
- Deleting the only Workspace clears its Notes and resets that same Workspace to the default name rather than leaving zero Workspaces.
- When deleting the active Workspace among several, select the most recently updated surviving Workspace.
- Persist active Workspace identity as application preference, not domain content.
- Database-open and migration failures render a recovery screen with error category and paths to retry or quit; do not auto-reset or overwrite.

## Acceptance criteria

- [ ] The thinker can create, select, rename, and confirm-delete a Workspace.
- [ ] Names are trimmed, validated, and Unicode-safe; duplicates remain distinct.
- [ ] The selected Workspace survives restart when it still exists.
- [ ] Deleting an active Workspace selects the defined survivor.
- [ ] Deleting the only Workspace leaves one empty valid default Workspace.
- [ ] Deletion atomically removes all child Notes already supported by the schema.
- [ ] Canceling confirmation changes nothing.
- [ ] A database-open failure never creates or overwrites a database and presents retry and quit actions.
- [ ] All lifecycle outcomes persist after adapter close/reopen.

## Testing decisions

- Exercise lifecycle only through Thinking Workspace intents.
- Test duplicate names, whitespace-only names, long Unicode names, cancel, only-Workspace delete, active-Workspace delete, and restart selection.
- Inject open and migration failures and prove no reset occurs.
- Extend SQLite/in-memory conformance coverage for lifecycle semantics.

## Blocked by

- #2

## Scope fence

Do not add Note editing beyond the foundation, Labels, Relationships, views, AI, archives, backups, or broad recovery tooling. This issue owns lifecycle and safe open failure only.

## Delivery workflow

Run `prd-simplicity-audit`, use a fresh `implement` session, then `code-review`, scoped `fallow`, focused tests and repository gates, and one PR against `main`.
