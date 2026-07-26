# PRD SIMPLICITY AUDIT

Feature: R11 — Background enrichment presence
Date: 2026-07-26
Gate: PROCEED

---

## MODULE MAP

| Module | Current responsibility and interface |
| --- | --- |
| `src/enrichment-controller.ts` | Owns the enrichment timer, active Note id, request lifecycle, and status; exposes `status` plus existing retry/replace actions. |
| `src/App.tsx` | Wires controller state to the active Thinking Workspace, shared Note card, and top bar. |
| `src/note-card.tsx` | Renders one Note and its existing quiet enrichment/recovery affordances; receives status and intent callbacks. |
| `src/styles.css` | Defines presentation only, including reduced-motion behavior. |
| `src/workspace-client.ts` | Provides the sole frontend seam to durable commands and `assistanceEnabled`. |
| `src-tauri/src/workspace.rs` | Owns the durable enrichment revision guard and stale-response rejection. |

No ADR directory exists in this repository. The established durable decision is the `thinkingWorkspace` client seam and the glossary in `CONTEXT.md`.

---

## INTERROGATION FINDINGS

### Note shimmer and top-bar presence

**CLEAN.** Both are pure projections of controller-owned status. The card receives status only for the controller's active Note; the top bar derives working from that same status. Neither view writes status or reaches into a command.

### Manual Workspace gating

**CLEAN.** `assistanceEnabled(activeWorkspace)` is already the single policy predicate. Rendering no presence when it is false prevents a separate, potentially divergent policy check.

### Inline failure recovery

**CLEAN.** The existing Note-card recovery actions continue to call controller methods. No modal, command, or failure state is introduced.

### Editing during organization

**CLEAN.** Presentation does not disable any editor. The existing Rust conformance test owns revision capture and stale-response rejection, so the durable guard remains in one module.

---

## COMPLEXITY SCORECARD

State Surface: Low — no new state; the existing controller status remains the sole transient owner.

Seam Quality: Preserved — React consumes the controller and durable behavior remains behind `thinkingWorkspace`/Rust.

Module Cohesion: Cohesive — status presentation belongs to card/top-bar components; lifecycle belongs to the controller.

Change Blast Radius: Narrow — App wiring, Note-card presentation, styles, and focused tests.

Incidental Complexity Load: Mostly Problem — the only new behavior is an observable projection of an existing request lifecycle.

Summary: This is a presentation-only slice over an established controller and revision guard. Keeping status ownership in the controller avoids cross-view mutable state while the active Thinking Workspace policy remains the one gate.

---

## GATE DECISION: PROCEED

Render quiet status from the existing enrichment controller, preserve its recovery actions and Rust revision guard, and add no configuration or lifecycle path to the main view.
