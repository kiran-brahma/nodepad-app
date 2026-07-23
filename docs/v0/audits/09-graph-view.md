# PRD SIMPLICITY AUDIT

Feature: V0-08 — Reconnect graph view and Relationship focus (GitHub issue #9)  
Date: 2026-07-23  
Gate: PROCEED

---

## MODULE MAP

- `src/workspace-client.ts` — the UI's only durable-state interface. `Relationship` already carries a canonically ordered symmetric pair and a Workspace, and `WorkspaceSnapshot` already returns every Relationship. Unchanged by this slice; no new command, no migration.
- `src/thinking-graph.ts` — today: related lookup, degree, and relate candidates, each computed independently from the raw `Relationship[]`. This slice makes it produce one projection — nodes and links — and derives related, degree, and the focus set from that one value.
- `src/note-focus.ts` — today: which Note the thinker navigated to, plus the scroll/focus effect and the rule that focus is let go when the Note leaves the screen. Grows hover, a lock toggle, Escape, and the lit set every view dims against.
- `src/note-views.ts` — the view roster and the one ordered result set. Gains `graph` as a third way of reading the same committed Notes.
- `src/graph-layout.ts` — new. A pure function from the graph projection to placed nodes and links, computed by running a d3 force simulation to rest and reading the result. No coordinate leaves this call.
- `src/graph-view.tsx` — new. Draws the placed graph and hosts the same Note card the other views place.
- `src/note-card.tsx`, `src/committed-notes-section.tsx`, `src/App.tsx` — the card gains a dimmed state and reads its degree from the shared projection; the section gains a third layout; App builds the one graph and hands it around.
- `components/graph-area.tsx`, `components/graph-detail-panel.tsx` — the pre-durable graph, over a `TextBlock` shape with `influencedBy` that no longer exists. Reference for behavior, not code this slice imports.

Both blockers are merged into `main`: #6 as `93dca1a` and #8 as `956aa7b`. This repository has no `docs/adr/`; `CONTEXT.md` is the domain authority.

---

## INTERROGATION FINDINGS

### Three readings of the same numbers — COMPLECTED, resolved

The issue asks that degree and related highlighting agree for all Notes. Today they cannot be made to disagree only because they happen to be written the same way: `degree` counts endpoints in `Relationship[]`, `relatedNotes` filters `Note[]` by those endpoints, and the graph would count a third time. Three counts of one thing is the definition of complected state — a dangling endpoint would raise the badge without adding a chip, and a re-render could show a node with an edge count no link matches.

The resolution is one value. `thinkingGraph(notes, relationships)` returns the nodes and links of one Thinking Workspace; degree is the number of links touching a node, the related set is the other endpoints of those links, and the graph draws exactly those links. Agreement stops being a property to test and becomes one you cannot express the negation of.

### Dangling and cross-Workspace links — CLEAN by construction

A link is admitted only when both endpoints name a Note in the list the projection was given, and App gives it the active Workspace's Notes. A Relationship left over from a moved Note, or one naming another Workspace, has no node to attach to and is never built, so no view has to check for it. Canonical pairs are deduplicated on the way in as well, so a pair drawn twice is not reachable either.

### Layout as a derived value — CLEAN

D3's force simulation is a stateful animation loop in its usual use, which is the shape the issue forbids persisting. Running it to rest inside a pure function and returning the placements removes the loop entirely: `graphLayout` takes the projection, ticks a fixed number of times, and returns coordinates that are recomputed from committed state on every restart. There is no simulation to own, no coordinate to store, and nothing to reconcile after a mutation. It also makes the layout testable as an outcome — every Note placed once, inside the canvas — without asserting a coordinate, which is what the testing decisions ask for.

### Which Notes the graph shows — CAUTION, resolved

The other two views render the search-narrowed result set. The acceptance criteria say every active-Workspace Note appears once, so the graph reads the Workspace's Notes rather than the search result. This also keeps degree honest: a search must not make a Note look less connected than it is. Search still narrows tiling and kanban, which is where a search result is useful; the graph stays a picture of the Thinking Graph, which is what it is for.

### Hover versus lock — CLEAN

Two transient values, not one: a hovered Note and a locked Note. The focal Note is the hovered one when there is one and the locked one otherwise, which is exactly "hover previews, click locks" without a mode flag. Both are cleared when their Note leaves the screen, by the rule that already exists.

### "Until view change" — CAUTION, deviating deliberately

The issue lists a view change among the things that clear a locked focus. Two other accepted outcomes contradict that: tiling and kanban must dim against the same related set, which is only observable if focus survives the switch between them, and the shipped behavior from #8 — with a test asserting it — keeps the navigated-to Note selected across a view switch. Clearing on view change would regress that test and make the cross-view dimming criterion unobservable. Focus is therefore cleared by click-again, Escape, and the Note leaving the screen (deletion, search, Workspace switch), and it survives a view change. This is called out here rather than resolved silently.

### One card, one set of intents — CLEAN

The graph's detail surface is the same `NoteCard` the other views place, over the same intents object. Dimming is one boolean on that card, computed from the shared lit set, so the three views cannot drift in what a focused Note looks like or in what may be done to one. Nothing about focus or dimming is committed.

### Node radius and isolated Notes — CLEAN

Radius is a function of degree over the maximum degree in the graph, floored so a degree-zero Note is a visible node rather than a point. Nothing else reads it. A graph with no Relationship at all has every node at the floor, which is the disconnected case working rather than a special case.

### Change test — CLEAN

If the arrangement of the graph, its radii, or its forces change later, `graph-layout.ts` changes and nothing else does. If what counts as a link changes, `thinking-graph.ts` changes and every view follows, because they all read the one projection. Neither reaches the durable interface.

---

## COMPLEXITY SCORECARD

State Surface: Low — no durable state; two transient values (hovered, locked) replacing one  
Seam Quality: Improved — three independent counts collapse into one projection  
Module Cohesion: Cohesive — meaning in `thinking-graph.ts`, geometry in `graph-layout.ts`, drawing in `graph-view.tsx`  
Change Blast Radius: Narrow (one rewritten pure module, two new modules, three UI files)  
Incidental Complexity Load: Mostly Problem

Summary: The feature is a third way of reading Notes that already exist, over Relationships that are already committed. The one real risk is arithmetic drift — a degree, a chip list, and an edge each counting the same Relationship separately — and it is removed by construction rather than by tests. The one place the spec must bend is the "view change" clearing rule, which contradicts both the cross-view dimming criterion and shipped behavior.

---

## GATE DECISION: PROCEED

### Implementation constraints carried forward

1. `thinkingGraph(notes, relationships)` is the only place a Relationship becomes a link. Degree, related sets, relate candidates, dimming, and the drawn graph all read it.
2. A link exists only when both endpoints are Notes in the projection's own list, and each canonical pair appears at most once.
3. `graphLayout` is a pure function returning placements. No simulation, coordinate, or view choice is stored, committed, or held across a restart.
4. Focus is transient: hovered previews, click locks, click-again and Escape clear, and a Note leaving the screen clears it. It survives a view change, per the finding above.
5. One `NoteCard`, one intents object, three layouts. Dimming is a prop on the card, never a per-view rule.
6. Tests cover the projection and the layout as outcomes, and the views through the DOM. No test asserts a coordinate, a radius, a tick count, or an animation.
