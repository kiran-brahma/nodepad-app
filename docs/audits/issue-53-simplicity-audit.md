# PRD SIMPLICITY AUDIT

Feature: R12 — Suggested Relationships, surfaced inline
Date: 2026-07-26
Gate: PROCEED

---

## MODULE MAP

| Module | Responsibility |
| --- | --- |
| `src/enrichment-controller.ts` | Owns transient organization status and result lifecycle. |
| `src/enrichment-contracts.ts` | Carries validated suggested Note IDs from the organization result. |
| `src/note-card.tsx` / `src/canvas-view.tsx` | Render Note and spatial Thinking Graph projections. |
| `src/note-intents.ts` / `src/workspace-client.ts` | Own the existing Relationship command seam. |
| `src-tauri/src/workspace.rs` | Gates and persists durable organization fields and Relationships. |

---

## INTERROGATION FINDINGS

### Suggestions remain proposals

**CAUTION — resolved.** Existing enrichment persistence writes AI Relationships immediately. R12 requires the opposite. Keep validated `relatedNoteIds` in transient controller/UI suggestion state and stop persisting them during organization; accepting delegates to the existing `relateNotes` command.

### Canvas line and Note chip

**CLEAN.** Both consume one transient suggestion projection. The canvas writes nothing and the Note chip delegates to explicit handlers.

### Dismissal dedupe

**CLEAN.** A controller-owned in-session pair key isolates dismissal state without creating a durable Relationship or storage rule.

### Accepted suggestion

**CLEAN.** The existing command creates the normal Relationship; the transient proposal clears from the same pair.

---

## COMPLEXITY SCORECARD

State Surface: Low — one controller-owned, session-only suggestion set.

Seam Quality: Preserved — UI reaches durable state only through `thinkingWorkspace.relateNotes`.

Module Cohesion: Cohesive — controller owns proposals; canvas/card only render them.

Change Blast Radius: Medium — enrichment persistence, controller, canvas/card, focused tests.

Incidental Complexity Load: Mostly Problem — proposal state is required to prevent AI auto-linking.

Summary: The feature is structurally sound once proposal IDs stop becoming durable Relationships during organization. No new prompt, validation, or command seam is needed.

---

## GATE DECISION: PROCEED

Implement transient suggested Relationships over the existing organization result and explicit Relationship command.
