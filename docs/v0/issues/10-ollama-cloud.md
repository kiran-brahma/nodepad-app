## Parent

Part of #1.

## What to build

Add direct Ollama Cloud configuration, explicit per-Workspace consent, macOS keychain secret handling, and dynamic cloud model discovery. The endpoint is fixed to `https://ollama.com`; authentication is a bearer key; model listing uses Ollama's native tags endpoint.

## Decisions

- Cloud AI is a per-Workspace Assistance Policy, never a global implicit switch.
- Before first activation on each Workspace, disclose that the active Note and bounded relevant context may leave the Mac for inference; require affirmative consent.
- Store the bearer key only in the macOS keychain through a narrow secret seam.
- Never write the key to SQLite, browser storage, React persistence, logs, errors, archives, crash output, command-line arguments, or test snapshots.
- One application-level Ollama Cloud credential may be used by multiple consented Workspaces; consent remains per Workspace.
- Removing/revoking the credential moves affected Workspaces to a visible unavailable state without changing their durable Notes.
- Use `/api/tags`; treat IDs as opaque and never hard-code the cloud catalog.
- Switching away from Cloud AI invalidates outstanding cloud requests immediately.
- This slice does model discovery only; it sends no Note content.

## Acceptance criteria

- [ ] Cloud AI cannot activate until the Workspace disclosure is accepted.
- [ ] The key saves, reads, replaces, and deletes through macOS keychain behavior.
- [ ] Authenticated cloud tags can be fetched, refreshed, searched, selected, and persisted as non-secret model preference.
- [ ] Authentication failure, timeout, rate limit, malformed response, empty list, and retired/missing selected model are distinct.
- [ ] Removing consent or selecting Manual/Local invalidates pending cloud work and stops future cloud calls.
- [ ] Key material is absent from SQLite, serialized preferences, logs, rendered error text, and archive-ready state.
- [ ] Restart restores policy/consent/model preference and reads secret only when cloud access is needed.
- [ ] No custom cloud host or non-Ollama provider is introduced.

## Testing decisions

- Use a fake keychain adapter for automated contract tests and a narrow macOS integration test where available.
- Scan serialized state, database rows, logs, and error output for a sentinel key value.
- Use controlled HTTP fixtures for authenticated tags and every failure category.
- Test consent isolation between two Workspaces and cancellation on policy changes.

## Blocked by

- #10

## Scope fence

Do not organize Notes, call chat/generate, manage Ollama accounts or models, add custom hosts, or add another provider.

## Delivery workflow

Run `prd-simplicity-audit`, use a fresh `implement` session, then `code-review`, scoped `fallow`, focused tests and repository gates, and one PR against `main`.
