## Parent

Part of #1.

## What to build

Add the Assistance Policy foundation and first-party local Ollama discovery. A Workspace starts Manual. The thinker can select Local AI, connect only to `http://localhost:11434`, fetch and refresh models from Ollama's native tags endpoint, search the returned list, select a model, and understand unavailable or missing-model states.

This slice proves provider configuration and discovery but does not organize Notes yet.

## Decisions

- Assistance Policy is per Workspace: Manual, Local AI, or Cloud AI. This slice implements Manual and Local AI behavior plus the durable enum needed later.
- New Workspaces default to Manual.
- Local host is fixed to `http://localhost:11434`; no custom host field.
- Local Ollama requires no credential.
- Use Ollama's native `/api/tags` response, not OpenAI compatibility.
- Treat model identifiers as opaque strings. Do not hard-code a catalog or infer capability from brand names.
- Recognize cloud-capable models returned by the locally signed-in Ollama host without special authentication in Nodepad.
- Persist selected endpoint kind and model identifier as non-secret preferences.
- If a saved model disappears, show a typed missing-model state and require another selection; do not silently choose.
- Switching to Manual stops future provider calls and invalidates outstanding discovery work.

## Acceptance criteria

- [ ] Every new Workspace displays Manual Assistance Policy.
- [ ] The thinker can select Local AI and return to Manual.
- [ ] Nodepad fetches, displays, searches, refreshes, and selects models returned from local `/api/tags`.
- [ ] The selected model and policy survive restart.
- [ ] Unavailable host, timeout, malformed response, empty list, and missing selected model have distinct actionable states.
- [ ] Cloud-capable model names returned locally are selectable without a Nodepad cloud key.
- [ ] No Note content is sent and no organization prompt exists in this slice.
- [ ] No custom URL, credential, model management, or hard-coded provider catalog is introduced.

## Testing decisions

- Use controlled HTTP fixtures for tags success/failure; no live Ollama requirement in automated tests.
- Test stale refresh cancellation, policy switch, missing model after restart, opaque identifiers, Unicode model names, and response validation.
- UI integration covers Manual -> Local AI -> refresh/search/select -> restart -> Manual.

## Blocked by

- #4

## Scope fence

Do not call chat/generate, organize Notes, manage models, add direct Ollama Cloud, custom hosts, OpenRouter, OpenAI, Z.ai, or other providers.

## Delivery workflow

Run `prd-simplicity-audit`, use a fresh `implement` session, then `code-review`, scoped `fallow`, focused tests and repository gates, and one PR against `main`.
