# PRD SIMPLICITY AUDIT

Feature: V0-06 — Move and copy Notes safely between Workspaces (GitHub issue #7)  
Date: 2026-07-23  
Gate: PROCEED

---

## MODULE MAP

- `src-tauri/src/workspace.rs` — the Thinking Workspace interface, its SQLite adapter, and the in-memory conformance adapter. Owns durable intents, committed snapshots, undo history, and typed outcomes.
- `src-tauri/src/thinking_graph.rs` — the Thinking Graph rules. Already owns the endpoint rule a transfer needs; unchanged by this slice.
- `src-tauri/migrations/` — no migration. Moving a Note changes a column value, not a schema.
- `src/workspace-client.ts` — the UI's only durable-state client.
- `src/note-transfer.ts` — new. Choosing a destination and saying what each of the two transfers does, as pure functions over committed state.
- `src/App.tsx` — the Note detail surface, which grows one destination chooser with two clearly separate commands.

Both blockers are merged into `main`: #3 as `dc95969` and #6 as `6bfd5d5`. No relevant ADR exists under `docs/adr/`.

---

## INTERROGATION FINDINGS

### One mutation shape for a move — CLEAN

A move and the undo of a move are the same operation pointed in opposite directions: put this Note in that Workspace, with these Label meanings and these Relationships. Expressing it as one `NoteMutation::Relocate` variant keeps undo an ordinary committed mutation instead of a second code path, and the "restore only Relationships whose endpoints are still there" rule becomes one filter inside that variant rather than a rule the undo caller enforces.

### Label remapping — CLEAN

Labels already carry a canonical name and a per-Workspace uniqueness constraint, so "map by display meaning, create only when missing" is a lookup the Labels slice already owns. Extracting it as `label_id_for` gives `attach_label` and both transfers one implementation, so a case variant can only ever be reused, never duplicated.

The rule that a Note's Label membership travels with the Note row now also covers `Insert`, which closes a real gap: before this slice, undoing a Note delete restored the Note in SQLite without its Labels, while the in-memory adapter restored both. Conformance now means the same thing in both adapters.

### Copy as an insert — CLEAN

A copy is a new Note with the authored fields of an old one. It needs no new mutation shape, no new undo rule, and no notion of a link back to its source. Undoing it is the delete that already exists.

### Orphaned Labels after a move — CAUTION, accepted

A moved Note can leave a Label in its source Workspace with no Note carrying it. Deleting the orphan (as `detach_label` does) would make an undone move re-create the Label under a fresh identity, so a move followed by its undo would not return the thinker to where they started. Keeping the row makes the round trip exact and costs nothing visible, since Labels are only ever displayed through the Notes that carry them. Documented at `write_note_labels`.

### Which Workspace owns the undo — CAUTION, decided

Undo history is per Workspace, and a transfer touches two. Filing the command under the Workspace the Note came from matches what the thinker sees: they act in the source Workspace, watch the Note leave, and press Undo there. This needed one new seam, `commit_note_in`, so a command can name its own history without the mutation implying it.

### Relationships and the seam — CLEAN

Nothing has to decide whether a Relationship may cross a Workspace: a relocation removes every Relationship touching the Note and re-adds only those whose two endpoints are Notes in the destination. Both halves of a move satisfy that with the same code, and the schema's cascade still guarantees no dangling endpoint.

### Two commands, not one mode — CLEAN

A move and a copy have different outcomes for the Note the thinker is looking at, so the interface names both and explains both before either is committed. One button with a mode would make the more destructive of the two reachable by mistake.

---

## COMPLEXITY SCORECARD

State Surface: Low — no new table, no new column, one new mutation shape  
Seam Quality: Preserved — one durable interface, one new pure UI module  
Module Cohesion: Cohesive  
Change Blast Radius: Narrow (Workspace adapter and conformance, two commands, client, one UI path)  
Incidental Complexity Load: Mostly Problem

Summary: Every rule the issue asks for is expressible over state that already exists. The only genuinely new decision is which Workspace owns a transfer's undo, and it has one defensible answer.

---

## GATE DECISION: PROCEED

### Implementation constraints carried forward

1. A move and its undo share one mutation shape; neither direction gets its own code path.
2. Label lookup by display meaning has one implementation, shared with `attach_label`.
3. No Relationship is written by a transfer except by restoring one an undone move captured, and only while both endpoints are Notes in the Workspace it returns to.
4. Every refusal — a missing Note, a missing Workspace, the same Workspace — is decided before a transaction opens.
5. A transfer's undo belongs to the Workspace the Note came from, and the pull request says so.
