# PRD SIMPLICITY AUDIT

Feature: R13 — Synthesis as an ambient offer
Date: 2026-07-27
Gate: PROCEED

---

## MODULE MAP

| Module | Current responsibility and interface |
| --- | --- |
| `src/synthesis-controller.ts` | Owns the debounce timer, the request generation, and the one `SynthesisStatus`. `pending` is derived from the committed snapshot; `accept(id)` / `dismiss(id)` submit the existing Workspace commands. Nothing else decides what is waiting on the thinker. |
| `src/synthesis-contracts.ts` | The `proposeSynthesis` / `acceptSynthesis` / `dismissSynthesis` commands over the client seam. Eligibility, cooldown, pending cap, and the two-to-five source rule are enforced in Rust behind them. |
| `src/synthesis-section.tsx` | The panel form of the same offers: text, provenance, source previews, Accept/Dismiss, dimmed when stale. |
| `src/graph-view.tsx` | Draws the Thinking Graph. `provisionalMarks(pending, placements)` already places a pending Synthesis at the centroid of the source Notes it names, with dashed leaders, and drops one whose sources are not drawn. |
| `src/canvas-view.tsx` | The spatial projection. Derives lines from the graph and a transient position map (`canvasRelationships`, `suggestedCanvasLines`); owns drag state only. |
| `src/committed-notes-section.tsx` | Picks the view and passes it the one card. Already threads `pendingSyntheses` to the graph and `suggestions` to the canvas. |
| `src/workspace-client.ts` | The sole frontend seam. `PendingSynthesis { id, workspaceId, text, model, labels, sourceNoteIds, stale, … }`. |
| `src/App.tsx` | Wires the controller into both the graph and the panel. |

Durable decisions this slice must not re-litigate: the `thinkingWorkspace` client seam, the one Thinking Graph projection, the `CONTEXT.md` glossary (a Synthesis "may become a Note when accepted" — it is not one before that).

---

## INTERROGATION FINDINGS

### Rendering a pending Synthesis on the canvas

**CLEAN.** The canvas already computes a `positions` map on every render and already derives inert dashed geometry from it (`suggestedCanvasLines`). An offer card is the same derivation over `pending`: centroid of the source positions, leaders to each source. It introduces no state — no position is committed for it, nothing is stored, and dismissing it simply stops the next render from drawing it. The domain rule "a Synthesis has no place in the Thinking Graph" survives because the card is not a `[data-note-id]` element: it cannot be dragged, cannot be a relationship endpoint, and has no `canvasX`/`canvasY`.

### Placement rule now living in two views

**CAUTION → resolved by extraction.** `graph-view.tsx` already owns "a pending Synthesis sits at the centroid of the source Notes that are drawn, and is not drawn when none of them are". Restating that inside `canvas-view.tsx` would give the application two answers to where an offer belongs, and they would drift the first time either view changes its anchor.

Resolution: one pure module, `src/synthesis-placement.ts`, exporting `synthesisAnchors(pending, anchorOf)` where `anchorOf(noteId)` returns the drawn anchor point or `null`. The graph passes its layout placements, the canvas passes its card centres. Both get the same centroid, the same "no drawn source, no offer" rule, and the same leader list. This is a subtraction: the rule moves out of `graph-view.tsx` rather than being copied into a second view.

### Accept and dismiss from the canvas

**CLEAN.** No new command and no new state. `onAcceptSynthesis` / `onDismissSynthesis` are threaded from `App.tsx` through `CommittedNotesSection` to the canvas and land on the controller's existing `accept` / `dismiss`, which submit `acceptSynthesis` / `dismissSynthesis` like any other Workspace commit. Accepting creates the thesis Note in Rust; the canvas learns about it the same way it learns about every other commit — the next snapshot. The canvas therefore needs no optimistic bookkeeping, and a refused command leaves the offer standing.

### Two surfaces offering the same decision

**CAUTION → resolved.** The panel (`SynthesisSection`) and the canvas card both show Accept/Dismiss. This is not a second answer: both read `synthesis.pending` from the one controller and call the same two functions, so they cannot disagree, and one commit removes the offer from both. The canvas is one of four views; deleting the panel would leave kanban, tiling, and search with no home for an offer, which the issue does not ask for and lists as out of scope.

### Stale

**CLEAN.** `stale` is a field on the committed `PendingSynthesis`, decided in Rust. The canvas reads it and renders dimmed with Accept disabled — the same rule the panel already applies, expressed once per view as presentation, never recomputed.

### A Manual Workspace

**CLEAN.** A Manual Workspace never schedules an attempt, so `pending` is empty and the canvas draws nothing. No `aiEnabled` flag needs to reach the canvas: absence of offers is already the answer, and adding the policy as a second gate would complect the Assistance Policy with a drawing decision.

### Change test

If the offer's copy, dash, or card shape changes in 6 months, `canvas-view.tsx` and `styles.css` change. If the placement rule changes, `synthesis-placement.ts` changes and both views follow. If accept ever needs to do more, `synthesis-controller.ts` changes and no view does.

---

## COMPLEXITY SCORECARD

State Surface: Low — no new state at all. The offer is derived on every render from the committed snapshot and the transient position map.

Seam Quality: Preserved — accept and dismiss cross the existing client seam through the existing controller; no new command.

Module Cohesion: Cohesive — placement in one pure module, presentation in the view, commitment in the controller.

Change Blast Radius: Narrow — `synthesis-placement.ts` (new), `canvas-view.tsx`, `graph-view.tsx` (shrinks), `committed-notes-section.tsx`, `App.tsx`, `styles.css`, plus tests.

Incidental Complexity Load: Mostly Problem — the one avoidable complexity (a second copy of the centroid rule) is removed by extracting it rather than duplicating it.

Summary: The slice is presentation over state that already exists. Every input — the pending offers, their staleness, their source IDs — is committed state read through one controller, and every output is a commit through a command that already exists. The only structural risk was duplicating the placement rule into a second view, and that is resolved by moving the rule into one pure module both views read. Nothing durable is added, so nothing durable can drift.

---

## GATE DECISION: PROCEED

Implement as specified, with one recorded structural change the issue's own placement decision implies: the "centroid of the drawn source Notes" rule moves out of `graph-view.tsx` into one pure `synthesis-placement.ts` that both the graph and the canvas read.
