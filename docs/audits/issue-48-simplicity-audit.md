# PRD SIMPLICITY AUDIT

Feature: R7 — Draw Relationships on the canvas
Issue: kiran-brahma/nodepad-app#48
Date: 2026-07-26
Gate: **PROCEED**

---

## MODULE MAP

| Module | Current responsibility | Change |
|---|---|---|
| `src/thinking-graph.ts` | Projects committed Workspace Notes and Relationships into canonical, undirected links. | Read-only input for canvas lines. |
| `src/workspace-client.ts` | UI-only durable-state interface over Tauri commands. | Existing relationship commands remain unchanged. |
| `src/App.tsx` / `src/committed-notes-section.tsx` | Builds the shared Thinking Graph and selects a Note projection. | Wires existing relationship commands into the canvas. |
| `src/canvas-view.tsx` | Owns canvas layout and transient pointer drag state. | Owns transient link-drag state and line rendering. |
| `src/note-card.tsx` | Renders shared Note content and existing card intents. | Reused unchanged inside the canvas. |

## INTERROGATION FINDINGS

### Relationship creation and removal

**CLEAN.** The canvas raises only `relateNotes` and `unrelateNotes` through the existing client seam. Validation, provenance, duplicate no-op behaviour, and Workspace locality stay with their durable owner.

### Link gesture

**CLEAN.** Pointer origin, hover feedback, and drop target are transient canvas concerns. The gesture stores no durable relationship state and cancels invalid or empty drops before calling the client.

### Canvas line rendering

**CLEAN.** A read-only SVG layer consumes the shared Thinking Graph projection and current card positions. It does not construct a second relationship list or impose direction/type semantics.

### Focus treatment

**CAUTION — resolved.** Focus is transient and already owned by `useNoteFocus`. Pass only its selected Note ID to line rendering; do not persist or reinterpret focus in the canvas.

### Tests

**CLEAN.** Component tests exercise gesture and line actions at the existing client-facing callback seam, plus the Thinking Graph projection used to produce the line set.

## COMPLEXITY SCORECARD

**State Surface:** Low — one canvas-local link-drag value; Relationships remain in the existing durable owner.
**Seam Quality:** Preserved — no UI storage path or new client API is introduced.
**Module Cohesion:** Cohesive — canvas owns spatial interaction and presentation; Thinking Graph owns relationship projection.
**Change Blast Radius:** Narrow — canvas, view wiring, CSS, and focused tests.
**Incidental Complexity Load:** Mostly Problem — direct spatial linking requires pointer targeting and an SVG layer; routing, labels, direction, and AI suggestions remain excluded.

## GATE DECISION: PROCEED

No BLOCK items. Implement canvas-local link gesture and line projection while preserving the existing Thinking Graph and workspace-client seams.
