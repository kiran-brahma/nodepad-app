# PRD SIMPLICITY AUDIT

Feature: R6 — Canvas placement: draggable Notes with durable position
Issue: kiran-brahma/nodepad-app#47
Date: 2026-07-25
Gate: **PROCEED**

---

## MODULE MAP

| Module | Current responsibility | Change |
|---|---|---|
| `src-tauri/migrations/` | Ordered additive SQLite schema evolution. | Add one nullable position migration. |
| `src-tauri/src/workspace.rs` | Owns Note state, transactions, undo, SQLite/in-memory conformance, and snapshots. | Owns position mutation and position fields. |
| `src-tauri/src/lib.rs` | Exposes typed Tauri commands. | Exposes `set_note_position`. |
| `src/workspace-client.ts` | The UI's only durable-state interface. | Adds position fields and one command. |
| `src/App.tsx` / `src/committed-notes-section.tsx` | Selects and renders the active Note view using shared card intents. | Wires the new canvas view without changing card mutation ownership. |
| `src/note-card.tsx` | Renders the same Note card in each view from shared intents. | Reused as the canvas card content. |

## INTERROGATION FINDINGS

### Durable nullable coordinates and `set_note_position`

**CLEAN.** Position is unavoidable mutable state and belongs with the Note in `workspace.rs`. The existing transaction, snapshot, undo, SQLite, and in-memory seams remain the only owners. A nullable pair preserves old databases and tiling behaviour.

### Deterministic committed auto-placement

**CAUTION — resolved.** Layout derivation and persistence could be complected if each render chooses a different placement. Keep packing a pure deterministic helper ordered by the already stable visible Notes; the canvas component only identifies missing positions and commits each one through the existing client command. It neither owns durable state nor writes directly to SQLite.

### Canvas rendering and drag interaction

**CLEAN.** The canvas owns transient drag coordinates only. It renders the established `NoteCard` projection and calls the single position command exactly once on drop. This separates direct interaction from durable mutation. No zoom, pan, relationship routing, or multi-select state is introduced.

### View selection

**CLEAN.** A canvas is another projection beside tiling, kanban, and graph. The view preference remains transient in `App`; it does not become a new persisted concern.

### Tests

**CLEAN.** Rust conformance tests exercise the durable owner. React tests use the existing `thinkingWorkspace` seam and assert placement/one-drop commit rather than storage internals.

## COMPLEXITY SCORECARD

**State Surface:** Low — two nullable values per Note, written through one owner; drag coordinates are local and transient.
**Seam Quality:** Preserved — the existing client/Tauri command seam remains exclusive.
**Module Cohesion:** Cohesive — storage owns committed positions; the canvas owns display and gesture state.
**Change Blast Radius:** Medium — migration, storage, command/client contract, and a new view need coordinated type updates.
**Incidental Complexity Load:** Mostly Problem — durability and direct dragging require these boundaries; scope fences avoid unrelated canvas machinery.

## GATE DECISION: PROCEED

No BLOCK items. Implement a nullable durable position through the existing workspace seam, with deterministic committed auto-placement and a canvas-local drag gesture.
