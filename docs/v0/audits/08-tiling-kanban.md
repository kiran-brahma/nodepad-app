# PRD SIMPLICITY AUDIT

Feature: V0-07 — Reconnect tiling and kanban to durable projections (GitHub issue #8)  
Date: 2026-07-23  
Gate: PROCEED

---

## MODULE MAP

- `src/workspace-client.ts` — the UI's only durable-state interface. Already returns every field both views need (`pinned`, `noteType`, `labels`, `annotation`, `createdAt`) and every intent they must submit. Unchanged by this slice.
- `src/App.tsx` — owns the committed-Notes surface: every mutation handler, every transient draft, and the single Note card that renders one committed Note. Grows a view choice and two layouts around the card it already has.
- `src/note-controls.ts` — pure Note presentation helpers (`notePreview`, `noteTypeLabel`) shared by every surface that names a Note.
- `src/thinking-graph.ts` — Relationship reads for the card. Unchanged.
- `src/note-views.ts` — new. The projections both views consume: the active-Workspace result set, its pinned-first order, the tiling pages and their split tree, the kanban columns, and whether a selection survives a view switch. Pure functions over a committed snapshot.
- `components/tiling-area.tsx`, `components/kanban-area.tsx`, `components/tile-card.tsx` — the proven experiences from the pre-durable app. They read a `TextBlock` shape with `contentType`, `isEnriching`, `influencedBy`, and `timestamp` that no longer exists. They are the reference for behavior, not code this slice imports.
- `src-tauri/` — untouched. No projection this slice needs is missing from the committed snapshot, so there is no migration and no new command.

Both blockers are merged into `main`: #4 as `2f17282` and #5 as `e948d3b`. This repository has no `docs/adr/`; `CONTEXT.md` is the domain authority.

---

## INTERROGATION FINDINGS

### Layout as a function of the ordered result set — CLEAN

Tiling needs no durable state that does not already exist. A page is a slice of the ordered Notes, and a split tree is a recursive halving of that slice; both are values computed from the list, discarded on the next render, and identical after a restart because the order they read is committed. Nothing writes a coordinate, so there is nothing to migrate, reconcile, or repair. The deletion test passes: delete `note-views.ts` and the same halving logic reappears inside two view components, which is exactly the duplication the issue forbids.

### One result set, two views — CLEAN

Pinned-first with creation order breaking ties is already the order the durable interface returns and the order the current surface shows. Making that the single `visibleNotes` projection means the two views cannot disagree about order: they consume one array. A view that sorted for itself would complect display with the pin rule.

### Search as a filter, not a second list — CAUTION, resolved

`searchNotes` returns `SearchResult`, a different shape with its own snippet, Note Type, and Labels. Rendering that shape beside the Note cards is what "alternate copies of domain state" means: the same Note would exist twice on screen, and an edit would update one of them. The resolution is that a search result contributes only its `noteId`. The projection takes an optional set of matching ids, filters the committed Notes through it, and both views render Note cards throughout. Search then narrows what the thinker sees without ever becoming a second source of Note content.

### Kanban column order — CLEAN

`NOTE_TYPES` is a fixed ordered constant the durable interface already owns. Ordering columns by their position in it is deterministic, needs no table, and stays correct when a Note Type is added later. Only Note Types present in the current result set get a column, so an empty column can never appear and a filtered result cannot show a stale one.

### One mutation policy, one card — CLEAN

The card that renders a committed Note stays a single closure inside `App`, over the same handlers it uses today, and both layouts place that card. Extracting the card into a component with fifteen callbacks would make each view re-declare which intents it may submit, which is duplicated mutation policy wearing an interface. Layout decides where a Note appears; it never decides what may be done to one.

### Transient state stays transient — CLEAN

The view choice, the focused Note, and every draft live in component state and are never committed. Because the view choice is not persisted, a restart reconstructs both views from SQLite alone, which is the acceptance criterion stated positively. Selection survives a switch by the only rule that can be checked without durable state: keep the focused Note if it is still in the visible result, otherwise clear it.

### Task-typed sticky header and enrichment column — CAUTION, dropped

The proven tiling view lifted `contentType === "task"` into a sticky header, and the proven kanban view kept a synthetic "Enriching" column for AI work in flight. Neither has a referent in the durable model: `task` is one of fourteen equal Note Types, and enrichment is not in V0. Carrying them over would mean a view inventing a classification the domain does not have. Both are dropped; `task` becomes an ordinary column and an ordinary tile.

### Minimap and animation — CAUTION, deferred

The issue permits retaining them but does not require it. Both need scroll observers and layout measurement — transient machinery that earns nothing at this slice's Note counts and cannot be tested without asserting on pixels, which the testing decisions forbid. They are omitted here. Nothing in this slice's shape blocks adding them later, because a minimap reads the same page projection the view renders.

### Change test — CLEAN

If the tiling arrangement, page size, or column order changes in six months, `note-views.ts` changes and nothing else does: no schema, no command, no client, no mutation handler. That is the seam the issue asks for, stated as a blast radius.

---

## COMPLEXITY SCORECARD

State Surface: Low — no durable state at all; one transient view choice added  
Seam Quality: Preserved — one durable interface, one new pure UI module, one card  
Module Cohesion: Cohesive — `note-views.ts` computes arrangement and nothing else  
Change Blast Radius: Narrow (one new pure module, one UI file)  
Incidental Complexity Load: Mostly Problem

Summary: This slice adds arrangement, not meaning. Every value both views need is already committed, so the whole feature reduces to pure functions from an ordered Note list to pages and columns, plus a transient choice of which arrangement to show. The two real risks are letting search results become a second copy of Note content and letting each view carry its own mutation policy; both are avoided by construction — one filtered projection and one shared card — rather than by discipline.

---

## GATE DECISION: PROCEED

### Implementation constraints carried forward

1. `src/note-views.ts` holds every arrangement decision as pure functions over committed Notes; no view computes its own order, pages, or columns.
2. A search result contributes only a `noteId`. Note content is rendered from the committed Note in both views, never from a snippet.
3. One Note card, one set of mutation handlers, shared by both layouts; a layout never gains an intent the other lacks.
4. No coordinate, page index, column, or view choice is committed or persisted.
5. Kanban columns follow `NOTE_TYPES` order and appear only for Note Types present in the current result.
6. Tests cover projections as pure outcomes and the views through the DOM; no test asserts a pixel, a size class, or an animation.
