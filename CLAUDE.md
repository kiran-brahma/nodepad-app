# CLAUDE.md

Working notes for Claude Code in this repository. [AGENTS.md](AGENTS.md) is the process of record; this file is the practical detail underneath it.

## What this is

Nodepad: a native macOS thinking tool. Tauri 2 + React + TypeScript on the front, Rust + SQLite behind. No server, no browser storage, no telemetry. AI assistance is optional and per-Workspace.

## Before writing code

1. Read the complete issue.
2. Read [CONTEXT.md](CONTEXT.md). Use its vocabulary exactly — in code, tests, comments, commits, and PR text. Never the words listed under `_Avoid_` (no "project", "space", "block", "tile", "node", "edge", "connection").
3. Read any ADR under `docs/adr/` touching the area, and the audits in `docs/audits/` for issues near yours — they record structural decisions you must not re-litigate.
4. Run the `prd-simplicity-audit` skill and record the result at `docs/audits/issue-<n>-simplicity-audit.md`, matching the existing files' shape (Module map → Interrogation findings → Complexity scorecard → Gate decision). Do not widen scope.

## Commands

```bash
npm run check
```

The gate: `typecheck` → `lint` → `vitest` → `cargo fmt --check` → `clippy -D warnings` → `cargo test` → Tauri compile smoke. Takes a few minutes; run it in the background and keep working. `npm run build` = gate + macOS artifact. `npm run dev` launches the app.

Fast loops: `npx vitest run src/<file>.test.tsx`, `npx vitest run -t "<name>"`, `npx tsc --noEmit`.

## Architecture invariants

**Durable rules live in Rust.** Invariants — the one-valid-Workspace rule, Relationship validity, undo, Synthesis eligibility and cooldown — are enforced against SQLite in `src-tauri/src/`. The frontend never restates them; if you find yourself checking one in TypeScript, it belongs behind a command.

**One seam.** `src/workspace-client.ts` is the only path from the frontend to Rust, and the only test seam. Do not open another. Tests drive real controls through the DOM and assert on returned state (`src/App.test.tsx` and the focused `*-view.test.tsx` / `*-controller.test.ts` files are the prior art).

**One home per concern.** Each of these is the single place its subject is decided:

| Concern | Module |
| --- | --- |
| What may be done to a Note | `src/note-intents.ts` (`NoteIntents`) |
| What ⌘K offers | `src/palette-actions.ts` |
| Which Note is focused, and what it lights | `src/note-focus.ts` |
| Relationship projection (degree, lit set, candidates) | `src/thinking-graph.ts` |
| Which Notes are on screen, in what order | `src/note-views.ts` |
| Escape ordering across surfaces | `src/escape-stack.ts` |
| Modal focus trap and restore | `src/modal-focus.ts` |

A new surface calls into these. It does not re-derive them — two surfaces that count the same Relationship differently is the bug class this structure exists to prevent.

**Canonical lists are mapped, not restated.** `NOTE_TYPES`, `NOTE_VIEWS`, `ASSISTANCE_POLICIES`, `CLOUD_PROVIDER_LABELS`. If a UI enumerates one of them, `map` over the list so a new member cannot ship with a surface that has not heard of it.

**`App.tsx` is the orchestrator, and it is already large.** Wire things there; put logic in a module. Module-level builders (`buildNoteIntents`, `buildPaletteActions`) keep branching out of the component body — extend those rather than growing the component.

## Conventions

- **Comments explain intent, not mechanics.** The house voice states why a thing is the way it is, in the domain's language: "so ⌘K can never mean something different from clicking the card." Match the surrounding density; don't narrate code.
- Pure `.ts` modules ship a co-located `*.test.ts`. Interface behaviour goes through `App.test.tsx`.
- Prefer subtraction. A slice that removes a duplicate while adding a feature is the expected shape.
- Transient UI state (drafts, focus, open/closed) is not committed and must not be written to the snapshot.
- Every destructive action keeps its confirmation. A new path to delete something routes through the existing confirmation, never around it.

## Delivery

Per [AGENTS.md](AGENTS.md): audit → implement → `code-review` skill → `fallow` on new code only → focused tests → `npm run check` → scoped branch → one PR against `main`. Do not merge without review.

Commits and PR bodies are written in normal prose, not compressed. Commit trailer:

```text
Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
```

## Gotchas

- **This repo is a fork.** `gh pr create` targets the upstream unless you pass `--repo kiran-brahma/nodepad-app`. Same for `gh issue view`.
- GitHub shares one number space for issues and PRs; disambiguate with `gh pr view <n>` before falling back to `gh issue view <n>`.
- jsdom lacks `ResizeObserver` and `scrollIntoView`; `App.test.tsx` polyfills both at the top for cmdk. New test files that mount the palette need the same.
- The capture bar is `id="capture-bar"`, `aria-label="New Note"`. Older ids (`#note`) are gone — grep before targeting an element by id.
- `npm run check` includes Rust compilation; a 120s tool timeout will not cover it.
