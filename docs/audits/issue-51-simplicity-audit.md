# PRD SIMPLICITY AUDIT

Feature: R10 — Kanban drag to reclassify
Date: 2026-07-26
Gate: PROCEED

---

## MODULE MAP

| Module | Current responsibility and interface |
| --- | --- |
| `src/kanban-view.tsx` | Groups visible Notes into their existing Note Type columns and renders the shared Note card. |
| `src/note-views.ts` | Provides `kanbanColumns`, the ordered, non-empty Note Type projection. |
| `src/committed-notes-section.tsx` | Selects the current Note view and bridges view callbacks from App. |
| `src/App.tsx` | Owns snapshot refresh and passes durable mutations to committed views. |
| `src/note-intents.ts` | Owns the shared manual Note intents, including `setNoteType` through `thinkingWorkspace`. |
| `src/workspace-client.ts` | Defines the sole frontend interface to the Rust command surface; `setNoteType(noteId, noteType)` is already durable. |
| `src/App.test.tsx` | Exercises React view controls over a mocked `thinkingWorkspace` command surface. |

No ADR directory exists in this repository. The applicable durable decision is already expressed by the Thinking Workspace client seam and the domain glossary in `CONTEXT.md`.

---

## INTERROGATION FINDINGS

### Cross-column drag changes a Note Type

**CLEAN.** The Kanban view owns only the HTML drag-and-drop gesture. It receives a narrow callback and invokes the existing `setNoteType` path; it does not duplicate validation, persistence, provenance, undo, or snapshot ownership.

### Same-column drop is a no-op

**CLEAN.** The view can compare the dragged Note's current Note Type with its target column before invoking the callback. This creates no mutable state and preserves the durable command history from accidental changes.

### Columns remain non-empty and ordered

**CLEAN.** `kanbanColumns` remains the sole owner of the column projection. The refreshed shared snapshot naturally removes an emptied origin column and keeps the destination column and all other views consistent.

### Tests at the existing durable seam

**CLEAN.** App-level drag/drop tests can assert command-driven snapshot changes without adding a mock store or alternate mutation path.

---

## COMPLEXITY SCORECARD

State Surface: Low — no new durable or component state; the browser drag payload exists only for the gesture.

Seam Quality: Preserved — Kanban receives an explicit callback and App routes it to the established `thinkingWorkspace` client command.

Module Cohesion: Cohesive — interaction belongs to Kanban; manual Note mutation remains in the existing intent/client path.

Change Blast Radius: Narrow — Kanban view, committed-view callback wiring, App wiring, and focused tests.

Incidental Complexity Load: Mostly Problem — HTML drag-and-drop is the specified gesture; no new abstraction or state layer is added.

Summary: The issue adds one transient UI gesture over an established durable command. The projection, persistence, provenance, undo, enrichment scheduling, and cross-view consistency remain owned by their existing modules.

---

## GATE DECISION: PROCEED

Implement a cross-column Kanban drag that delegates only to the existing `setNoteType` command path, records the same-column no-op, and leaves `kanbanColumns` as the sole column projection.
