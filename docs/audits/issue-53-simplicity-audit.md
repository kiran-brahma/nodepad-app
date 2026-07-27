# PRD SIMPLICITY AUDIT

Feature: R12 — Suggested Relationships, surfaced inline
Date: 2026-07-27
Gate: PROCEED

---

## MODULE MAP

| Module | Current responsibility and interface |
| --- | --- |
| `src/thinking-graph.ts` | The one projection of committed Notes + Relationships into nodes and links. Degree, related sets, relate candidates, dimming, and every drawn line read this value. Writes nothing. |
| `src/enrichment-controller.ts` | Owns the debounce timer, request token, and the one `EnrichmentStatus`. The `applied` status already carries `result.relatedNoteIds` — the AI's proposal, already validated in Rust against the candidate set. |
| `src/canvas-view.tsx` | The spatial projection. `canvasRelationships(graph, positions, lit)` turns links into lines; drag state is transient. |
| `src/note-card.tsx` | Draws one Note wherever it appears. Holds no durable state; every change leaves through the one `NoteIntents` object. |
| `src/note-intents.ts` | Builds the one set of Note intents, including `relate` over `thinkingWorkspace.relateNotes`. |
| `src/workspace-client.ts` | The sole frontend seam over the Rust command surface. `relateNotes(noteId, otherNoteId)` is the one relate command; `RelationshipProvenance = 'manual' | 'ai'`. |
| `src-tauri/src/enrichment.rs` | Parses and gates an enrichment result. `gate_parsed_against_source` returns `ApplicableFields`, which today includes `add_relationships`. |
| `src-tauri/src/workspace.rs` | `persist_enrichment` commits those fields, **including inserting Relationships with `Ai` provenance**. |

No ADR directory exists. The applicable durable decisions are the `thinkingWorkspace` client seam, the one Thinking Graph projection, and the glossary in `CONTEXT.md`.

---

## INTERROGATION FINDINGS

### "Suggestions must never be auto-applied"

**COMPLECTED → resolved by removing, not adding.** The issue's premise is that a proposed Relationship is not yet a Relationship. The durable layer contradicts it: `persist_enrichment` inserts every gated `add_relationships` entry with `RelationshipProvenance::Ai` inside the same transaction as the Note Type, Annotation, and Labels. If that stays, every proposal is already a committed link by the time the frontend sees the result, the dedupe rule ("not already related") hides every chip, and the whole slice is unreachable code.

Resolution: delete the relationship channel from the enrichment application — `ApplicableFields.add_relationships`, `remove_relationship_ids`, the `existing_relationships` gate parameter, and the insert loops in both store implementations. The AI's proposal now leaves Rust only as `result.relatedNoteIds` on the outcome, which is data for the thinker to act on. This removes state and one write path; it adds none. The issue's stated interfaces ("this slice does not change the prompt or validation") are untouched: parsing, the candidate-set validation, and the five-item cap all stay.

Consequence recorded: `relateNotes` writes `manual` provenance, so an accepted suggestion is recorded as the thinker's act — which is what accepting it is. `RelationshipProvenance::Ai` remains a value the durable layer and the archive format read and carry; nothing writes it while the AI never links on the thinker's behalf.

### Where a suggestion lives

**CAUTION → resolved.** A suggestion is explicitly transient: dismissal "is not a committed Relationship". The temptation is a second durable table, or a per-Note map inside the enrichment controller. Both would give the application a second answer to "what links exist" alongside the Thinking Graph.

Resolution: one small session-scoped module, `src/suggested-relationships.ts`, holding two values — the proposals seen this session and the pairs dismissed this session — and one pure projection, `visibleSuggestions(graph, proposed, dismissed)`, which subtracts from the proposals everything the graph already answers: a pair whose endpoints are not both present, a pair already related, a pair dismissed. Accepting therefore needs no bookkeeping beyond dropping the proposal: the committed link arrives in the next snapshot and the graph removes the suggestion by itself. The controller keeps owning enrichment status; the graph keeps owning what is linked.

### Rendering as a dashed line and a chip

**CLEAN.** The canvas already derives lines from the graph and a position map; suggested lines are the same derivation over the suggestion list, drawn with a dashed accent class. The chip is a card element in the existing meta area, over two new intents (`acceptSuggestion`, `dismissSuggestion`) that follow the existing enrichment-callback pattern. No modal, no new layout surface, no second relate command.

### Which Note carries the chip

**CAUTION → resolved.** A pair has two endpoints; rendering a chip on both would double one proposal. A suggestion is recorded with the Note whose organization proposed it, so exactly one chip appears; the canonical pair key is used only for dedupe and dismissal, so the same pair proposed from the other side is still one suggestion.

### Dedupe within the session

**CLEAN.** One `Set` of canonical pair keys. Dismissal adds a key; the projection subtracts it. "Unchanged pair" is the pair identity, which is all the issue asks for.

### Change test

If the suggestion wording, the dash, or the chip changes in 6 months, `suggested-relationships.ts`, `note-card.tsx`, and `styles.css` change. If suggestions ever need to survive a restart, one module gains a store call and the projection stays. If enrichment concurrency changes, the controller changes and nothing here does.

---

## COMPLEXITY SCORECARD

State Surface: Low — two session values in one module, and one durable write path *removed* from Rust.

Seam Quality: Preserved — accepting goes through the existing `relateNotes` command; nothing new crosses the client seam.

Module Cohesion: Cohesive — proposal bookkeeping in one module, rendering in the card and the canvas, linking in the existing intent.

Change Blast Radius: `suggested-relationships.ts` (new), `canvas-view.tsx`, `note-card.tsx`, `note-intents.ts`, `committed-notes-section.tsx`, `App.tsx`, `styles.css`, plus `enrichment.rs` and `workspace.rs` shrinking by one write path, and tests.

Incidental Complexity Load: Mostly Problem — the one avoidable complexity (a durable or controller-owned suggestion store) was identified and replaced by a projection over the graph the application already has.

Summary: The slice is small once the contradiction under it is removed. The AI currently links on the thinker's behalf; the issue says it must not. Deleting that write path is a subtraction, not a widening of scope, and it is the precondition for every user story in the issue. Everything else is one projection, two intents, a dashed line, and a chip.

---

## GATE DECISION: PROCEED

Implement as specified, with one recorded change the issue's own problem statement requires: the enrichment application no longer commits Relationships, so a proposed link reaches the thinker as a suggestion rather than as a fact. Accepted suggestions are committed by `relateNotes` and carry `manual` provenance, because accepting is the thinker's act.
