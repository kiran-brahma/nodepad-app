# PRD SIMPLICITY AUDIT

Feature: R11 — Background enrichment presence
Date: 2026-07-26
Gate: PROCEED

---

## MODULE MAP

| Module | Current responsibility and interface |
| --- | --- |
| `src/enrichment-controller.ts` | Owns the debounce timer, the request token, the request generation, and the one `EnrichmentStatus` for the most recently scheduled Note. Exposes `status`, `activeNoteId`, `schedule`, `retry`, `requestReplace`, `confirmReplace`, `cancelReplace`, `cancel`, `clear`. Single-flight by construction. |
| `src/note-card.tsx` | Draws one Note wherever it appears; already receives `enrichment?: EnrichmentStatus` and renders `NoteEnrichmentBadge` in the meta line. Holds no durable state. |
| `src/note-intents.ts` | Builds the one set of Note intents, including the enrichment retry/replace callbacks App supplies. |
| `src/app-shell.tsx` | Layout only: rail, top bar, content, footer. Reads no state. |
| `src/App.tsx` | Orchestrator: computes `aiEnabled` via `assistanceEnabled(activeWorkspace)`, constructs the controller, and passes per-Note status into the card. Owns the top-bar region's contents. |
| `src/workspace-client.ts` | The sole frontend seam over the Rust command surface; `assistanceEnabled` is the one Assistance Policy gate. |
| `src/styles.css` | All presentation, including `.badge` and `.note .meta .badge`. |
| `src-tauri/src/workspace.rs` | Durable enrichment application, including the `enrichment_revision` guard (`apply_enrichment_rejects_stale_revision`). |

No ADR directory exists in this repository. The applicable durable decisions are the `thinkingWorkspace` client seam and the domain glossary in `CONTEXT.md`.

---

## INTERROGATION FINDINGS

### Shimmer on the Note being organized

**CLEAN.** The card already receives the active Note's `EnrichmentStatus`. A shimmer is a presentation of that value — one derived class name plus CSS. No new state, no new dependency, no reach across the client seam.

### One top-bar indicator, 'AI · quiet / working'

**CAUTION → resolved.** The issue words the indicator as "whether any enrichment is in flight for the active Workspace". The controller is single-flight: it holds exactly one `activeNoteId` and one `status`, and scheduling a second Note re-points the timer. Introducing a per-Note status map to answer "any in flight" would complect the presence indicator with a new mutable collection that nothing else needs, and would let the map and the controller disagree about what is running.

Resolution, recorded here and implemented: derive both the shimmer and the indicator from the single controller `status` through one pure function in a small `ai-presence` module (`status → "quiet" | "working"`). Same value, one owner, no new state. If enrichment ever becomes concurrent, the controller — not the top bar — is the module that changes.

### No AI presence in a Manual Workspace

**CLEAN.** `assistanceEnabled(activeWorkspace)` stays the one policy gate. Presence renders nothing when it is false; the gate is not re-derived from the policy enum anywhere in the view. Chosen: render nothing rather than a static "AI · off", which is the stronger reading of "Manual truly means manual".

### Inline, dismissible retry on failure

**CLEAN.** Retry and Re-enrich-and-Replace already exist as controller methods reached through `note-intents`. "Dismissible" maps onto the existing `clear()`, which cancels the generation and returns the status to `idle` — one new intent over an existing method, no new state and no modal.

### Non-blocking editing during `in_flight`

**CLEAN.** No new guard. Editing bumps `enrichment_revision` in Rust and the existing `apply_enrichment` check rejects the stale response. This slice consumes the contract and adds a conformance test that the invalidating edit is a *text edit during inference*, not only the pinned-flag edit the existing test uses.

### Restyled badge states

**CLEAN.** `.badge` presentation is owned by `styles.css`. The badge's state machine (organizing / applied / failed / replace-pending) is unchanged; only its typography and weight change.

### Change test

If the presence wording, the shimmer, or the indicator's states change in 6 months, `ai-presence.tsx` and `styles.css` change. If enrichment concurrency changes, `enrichment-controller.ts` changes and the presence module keeps its interface.

---

## COMPLEXITY SCORECARD

State Surface: Low — no new durable or component state. The one status the controller already owns is read, not duplicated.

Seam Quality: Preserved — the `thinkingWorkspace` client and the enrichment command contract are untouched; `assistanceEnabled` remains the single policy gate.

Module Cohesion: Cohesive — presence derivation and rendering live in one small module; the card renders a class; the controller keeps owning status.

Change Blast Radius: Narrow — `ai-presence.tsx` (new), `note-card.tsx`, `note-intents.ts`, `App.tsx`, `styles.css`, plus tests and one Rust conformance test.

Incidental Complexity Load: Mostly Problem — the feature is presentation of an existing value; the only avoidable complexity (a per-Note status map) was identified and removed before implementation.

Summary: This slice is a read of state that already has one owner. The single structural risk was the phrase "any enrichment in flight", which invites a second source of truth alongside the single-flight controller; the audit resolves it by deriving presence from the controller's own status through one pure function. Everything else is class names, CSS, and one intent over an existing controller method. No product outcome in the issue is changed by the resolution.

---

## GATE DECISION: PROCEED

Hand the issue to the implementation agent as written, with one recorded simplification: presence is derived from the controller's single `EnrichmentStatus` through a pure `ai-presence` function, not from a new per-Note status collection.
