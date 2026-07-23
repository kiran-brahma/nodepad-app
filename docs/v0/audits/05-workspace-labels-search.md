# PRD SIMPLICITY AUDIT

Feature: V0-04 — Workspace Labels and active-Workspace search (GitHub issue #5)  
Date: 2026-07-23  
Gate: MODIFY

---

## MODULE MAP

- `src-tauri/src/workspace.rs` — the Thinking Workspace interface and SQLite adapter; owns durable Workspace and Note intents, migrations, committed snapshots, and typed outcomes.
- `src/workspace-client.ts` — the UI's only durable-state client; maps UI intents to Tauri commands and receives committed snapshots/outcomes.
- `src/App.tsx` — current Workspace UI projection; it must consume committed Label and search outcomes rather than access SQLite.
- `src-tauri/migrations/` — ordered, transactional SQLite schema changes.
- `claude/github-issue-4-72d2df` — closed issue #4's unmerged implementation branch. It adds fixed Note Types, Annotation, manual provenance, and note-control intent seams required by this issue.

No relevant ADR exists under `docs/adr/`.

---

## INTERROGATION FINDINGS

### Label identity, lifecycle, and membership — CLEAN

Labels are a single Thinking Workspace concern when creation, attachment, detachment, rename, cleanup, and membership uniqueness are owned by one explicit Workspace intent boundary. The mutable state is durable but contained by SQLite transactions. A Label identity module earns its keep: without it, normalization and collision handling would spread into UI callers and search.

### Unicode normalization and preserved display spelling — CAUTION

The desired identity comparison is intentionally platform-sensitive. The implementation must establish one canonical Label key at the Thinking Workspace boundary, use it for every lookup and uniqueness constraint, and retain a separate display spelling. UI code must not normalize independently. This resolves the SQLite case-folding fallback without changing the product outcome.

### Detach, remove, and rename-to-merge — CLEAN

The lifecycle has unavoidable state transitions, but each can be one SQLite transaction. Rename collision must move memberships with uniqueness protection, delete the duplicate identity, and commit or roll back as one operation. That keeps the change local to the Workspace module.

### Active-Workspace FTS search — CAUTION

Search is a read projection, not a UI persistence concern. The Workspace module should own query preparation, FTS escaping/tokenization, Workspace filtering, deterministic rank tie-breaking, safe snippet generation, and omission of deleted Notes. Returning a typed projection prevents React from depending on SQL schema or FTS query syntax.

### Search scope and empty query behavior — CLEAN

The active Workspace is an explicit input; empty search restores transient UI state and writes nothing. This preserves the Workspace boundary and avoids a global-search state module.

### UI path — CLEAN

One labeling/search path is sufficient. It should submit intent or read-projection requests via `workspace-client.ts` and render committed outcomes; it must not issue direct UI SQL or duplicate the identity rules.

### Required baseline — BLOCK

Issue #5 is explicitly blocked by #4. Although #4 is closed, its implementation is commit `1393968` on `claude/github-issue-4-72d2df`, while `main` does not contain that commit. Building #5 from the current `main` would either duplicate #4's Note Type/Annotation/provenance work or couple Labels/search to a temporary branch. Both create incidental complexity and invalidate the required dependency order.

---

## COMPLEXITY SCORECARD

State Surface: Low  
Seam Quality: Preserved, once the #4 baseline is integrated  
Module Cohesion: Cohesive  
Change Blast Radius: Narrow (SQLite adapter, Workspace client, one UI path)  
Incidental Complexity Load: Mostly Problem

Summary: The feature has a good structural boundary: one durable Label identity and membership concern plus an active-Workspace search read projection. The only material risk is delivery order. Until the closed #4 implementation is available on `main`, this issue cannot use the intended interface without either reimplementing or reaching around it.

---

## GATE DECISION: MODIFY

### Modification Brief

1. Integrate issue #4's implementation into `main`, or provide an equivalent reviewed `main` baseline, before creating the issue #5 branch. This makes fixed Note Types, Annotation, manual provenance, and their shared Workspace interface real dependencies rather than copied code.
2. Specify the Label canonical-key operation as the single Workspace-module normalization seam, with display spelling stored separately. This prevents SQLite capability differences from leaking to callers.
3. Specify search as an active-Workspace typed read projection that owns FTS query safety, snippets, and deterministic tie-breaking. This preserves the no-direct-UI-SQL acceptance criterion.

Do not begin implementation until item 1 is complete. Items 2 and 3 are implementation constraints that preserve the issue's accepted outcomes.
