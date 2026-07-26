# PRD SIMPLICITY AUDIT

Feature: R9 — First-class view switcher
Issue: kiran-brahma/nodepad-app#50
Date: 2026-07-26
Gate: **PROCEED**

---

## MODULE MAP

| Module | Current responsibility | Change |
|---|---|---|
| `src/note-views.ts` | Defines `NOTE_VIEWS`, `NoteView`, labels, and shared view logic. | Remove `"tiling"` from `NOTE_VIEWS`; reorder to `["canvas", "kanban", "graph"]`; update `noteViewLabel`. |
| `src/committed-notes-section.tsx` | Renders committed Notes in the chosen arrangement, with view toggle buttons and Undo. | Remove view toggle buttons (move to top bar); remove tiling rendering path; remove `onChooseView` prop. |
| `src/app-shell.tsx` | Three-region layout shell with empty `app-main-topbar`. | Add `topbar` prop; render it in `app-main-topbar`. |
| `src/App.tsx` | Orchestrator: wires state, sections, and commands. | Change default view from `"tiling"` to `"canvas"`; add per-Workspace transient view state (`Map<string, NoteView>`); wire segmented control into top bar; add crossfade transition state; remove `view-tiling` palette action. |
| `src/styles.css` | All visual styles. | Add `.seg` segmented control styles; add crossfade transition class. |
| `src/App.test.tsx` | App-level tests. | Update default view assertion; add view switching and per-Workspace persistence tests. |

## INTERROGATION FINDINGS

### Segmented control in the top bar

**CLEAN.** The view toggle moves from `CommittedNotesSection` to the `app-main-topbar` via a new `topbar` prop on `AppShell`. This is a pure layout change: the toggle's behaviour (calling `setView`) is unchanged, only its position moves. The `AppShell` remains a layout-only component — it receives the control as a `ReactNode` and does not know what it is.

### Canvas as default view

**CLEAN.** Changing the initial state from `"tiling"` to `"canvas"` is a one-line change in `App.tsx`. No other module needs to know about the default.

### Removing "tiling" from NOTE_VIEWS

**CLEAN.** `"tiling"` is removed from the `NOTE_VIEWS` array and its label entry is deleted. Tiling remains as the canvas's internal auto-layout fallback (`autoCanvasPositions`), not as a separately labelled view. The `CommittedNotesSection` no longer renders a `<TilingView>` path. The `buildPaletteActions` function drops the `view-tiling` action.

### Crossfade transition

**CLEAN.** A brief opacity transition on the view container. The existing `prefers-reduced-motion` media query already neutralizes all transitions globally, so no additional reduced-motion handling is needed. The transition is implemented as a two-phase fade: fade out → swap content → fade in, using a local `useState` timer. This is self-contained in `App.tsx` and touches no other module.

### Per-Workspace transient view persistence

**CLEAN.** A `Map<string, NoteView>` in component state, keyed by Workspace id. When the active Workspace changes, the view is read from the map (defaulting to `"canvas"`). When the view changes, it is written to the map. No snapshot or durable storage is involved. The map is local to `App.tsx` and is not exposed to any child component.

### Tests

**CLEAN.** Tests exercise the segmented control rendering, view switching, default view, and per-Workspace persistence at the existing `App` component seam. No new test infrastructure is needed.

## COMPLEXITY SCORECARD

**State Surface:** Low — one `Map<string, NoteView>` for transient per-Workspace state, one `useState` for crossfade phase. Both are local to `App.tsx`.
**Seam Quality:** Preserved — `AppShell` gains a `topbar` prop but remains layout-only. `CommittedNotesSection` loses the view toggle and `onChooseView` prop, simplifying its interface.
**Module Cohesion:** Cohesive — each change is owned by exactly one module. The segmented control is a presentational concern in the top bar; view state is an orchestration concern in App.
**Change Blast Radius:** Narrow — `note-views.ts`, `committed-notes-section.tsx`, `app-shell.tsx`, `App.tsx`, `styles.css`, and focused tests.
**Incidental Complexity Load:** Mostly Problem — the crossfade and per-Workspace persistence are inherent to the feature. No incidental wiring or abstraction is introduced.

## GATE DECISION: PROCEED

No BLOCK items. Implement the segmented control in the top bar, canvas as default, crossfade transition, and per-Workspace transient view state while preserving the existing module boundaries.
