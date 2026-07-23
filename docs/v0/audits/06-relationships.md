# PRD SIMPLICITY AUDIT

Feature: V0-05 — Symmetric Relationships and Thinking Graph invariants (GitHub issue #6)  
Date: 2026-07-23  
Gate: PROCEED

---

## MODULE MAP

- `src-tauri/src/workspace.rs` — the Thinking Workspace interface, its SQLite adapter, and the in-memory conformance adapter. Owns durable intents, migrations, committed snapshots, and typed outcomes.
- `src-tauri/src/thinking_graph.rs` — new. The Thinking Graph module: canonical pair ordering, related lookup, degree, and pair validation as pure rules over committed state.
- `src-tauri/migrations/` — ordered, transactional SQLite schema changes.
- `src/workspace-client.ts` — the UI's only durable-state client.
- `src/thinking-graph.ts` — new. The same Thinking Graph projections the Note detail surface needs: related Notes, degree, and relatable candidates.
- `src/App.tsx` — the Note detail surface that adds, inspects, navigates to, and removes a Relationship.

No relevant ADR exists under `docs/adr/`. Issue #6's blocker, #4, is merged into `main` as commit `1393968`, and #5's Labels/search work is merged as `b08b6d8`, so the required baseline is real rather than branch-local.

---

## INTERROGATION FINDINGS

### Canonical pair storage — CLEAN

One unordered pair stored once is a single fact with a single owner. Sorting the two Note ids at the Thinking Graph boundary and constraining `note_id_a < note_id_b` in the schema makes "reversed endpoint order" stop being a case anything downstream must handle. Related lookup then reads both columns and never asks which endpoint is which. Nothing is complected: symmetry lives in one function and one constraint.

### Duplicate creation — CLEAN

The issue allows idempotence or a typed conflict. Idempotence is simpler: it removes a failure code, removes a UI error path, and matches what the thinker meant. The module guards on committed state and the unique index guards the database, so a duplicate can never exist even if the guard is raced.

### Self and cross-Workspace Relationships — CLEAN

Both are validation, not storage errors, and both are decidable from committed Notes before any transaction opens. Rejecting them before the transaction is what makes "fails without partial state" free rather than something to engineer.

### Cascade and dangling endpoints — CLEAN

`ON DELETE CASCADE` on both endpoints and on the Workspace, with `PRAGMA foreign_keys = ON` already set at open, makes a dangling endpoint unrepresentable rather than something projections must defend against. The in-memory adapter must mirror the cascade so conformance means the same thing in both adapters.

### Relationships and undo — CAUTION

Undo is a bounded per-Workspace log of `NoteMutation`s. Relationships are not Note rows, so Labels already sit outside undo and Relationships would too. Extending the mutation vocabulary to carry a deleted Note's Relationships would complect the graph with the undo log for a case the issue does not ask about. Accept the limitation and state it: undoing a Note delete restores the Note, not the Relationships that cascaded with it. This matches the existing Label behavior rather than inventing a second rule.

### Provenance — CLEAN

Note provenance is `default | manual`; Relationship provenance is `manual | ai`. They are different vocabularies with different valid sets, so one shared enum would let code build a value the schema rejects. A separate `RelationshipProvenance` keeps each check honest and satisfies "the schema must allow later AI provenance" without writing an AI row now.

### Navigation and focus — CLEAN

Focus is transient React state. It is not in the snapshot, not in the schema, and not an intent. Keeping it out of the durable interface is what stops navigation from being able to mutate a Relationship at all.

### The relation editor — CLEAN

Candidate selection is a pure projection over committed Notes and Relationships: same Workspace, never this Note, never one already related, narrowed by the thinker's text. It reuses no SQL and needs no new durable state.

---

## COMPLEXITY SCORECARD

State Surface: Low — one new table, one new snapshot field, one transient focus id  
Seam Quality: Preserved — one durable interface, one graph rules module per layer  
Module Cohesion: Cohesive  
Change Blast Radius: Narrow (migration, Workspace adapter and conformance, client, one UI path)  
Incidental Complexity Load: Mostly Problem

Summary: Relationships are one fact between two Notes with one canonical representation. The invariants the issue asks for are all expressible as schema constraints plus pure rules, so almost nothing has to be defended at runtime.

---

## GATE DECISION: PROCEED

### Implementation constraints carried forward

1. Canonical ordering and pair validation live only in the Thinking Graph module; no caller sorts endpoints or checks Workspace membership itself.
2. Duplicate creation is idempotent, guarded in the module and again by a unique index.
3. Relationship provenance is its own vocabulary, defaulting to manual, with AI admitted by the schema and unwritten by this slice.
4. Relationships stay outside undo history, exactly as Labels do. Note the consequence in the pull request rather than growing the mutation vocabulary.
