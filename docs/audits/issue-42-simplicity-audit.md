# PRD SIMPLICITY AUDIT

Feature: R1 — Three-pane app shell
Issue: kiran-brahma/nodepad-app#42
Date: 2026-07-25
Gate: **PROCEED**

---

## MODULE MAP

### Existing modules the PRD touches

| Module | Current responsibility | What changes |
|--------|----------------------|--------------|
| `src/App.tsx` | V0 orchestrator. Renders every section in a single scrolling `<main>` column. Owns all state, all event handlers, all section wiring. | Composes a new layout component instead of rendering sections directly. No state or handler changes. |
| `src/workspace-section.tsx` | Renders the workspace list and create form. Pure presentational. | Moves into the left rail region. No interface changes. |
| `src/capture-section.tsx` | Renders workspace controls (rename, delete, export) and the note capture form. Pure presentational. | Moves into the footer capture bar region. No interface changes. |
| `src/committed-notes-section.tsx` | Renders committed Notes in the chosen view (tiling/kanban/graph). Pure presentational. | Moves into the main pane's scrollable area. No interface changes. |
| `src/styles.css` | All styles. Currently uses `main { max-width: 760px; margin: 0 auto; }` for the single-column layout. | Adds shell layout styles (rail, main pane, footer). Existing section/card styles untouched. |

### Existing modules the PRD does NOT touch

- `src/assistance-section.tsx` — unchanged, rendered in main pane scrollable area
- `src/search-section.tsx` — unchanged, rendered in main pane scrollable area
- `src/synthesis-section.tsx` — unchanged, rendered in main pane scrollable area
- `src/intro-video.tsx` — unchanged, rendered in main pane scrollable area
- `src/note-card.tsx` — unchanged (explicitly out of scope per PRD)
- `src/workspace-client.ts` — unchanged (durable seam, explicitly preserved)
- All controllers, hooks, and test infrastructure — unchanged

### New modules

- **`src/app-shell.tsx`** (or similar name) — a new top-level layout component that owns the three regions (rail, main pane, footer capture bar) and renders existing section components into them. Single responsibility: layout. No business logic, no state beyond what its parent passes.

---

## INTERROGATION FINDINGS

### Item 1: "Replace the single scrolling column with a fixed three-region shell"

**CLEAN.** This is a pure layout change. The three regions are independent of each other (rail does not depend on main pane content, footer does not depend on rail state). The existing sections are pure presentational components that receive all their data via props — they can be moved into any region without interface changes.

The PRD explicitly states "No durable interface changes; getSnapshot and all commands are unchanged" — this is the correct seam discipline.

### Item 2: "a new top-level layout component owns the three regions"

**CLEAN.** A new module with a single responsibility (layout) that composes existing modules. The deletion test passes: if you deleted this component, the complexity would spread back into App.tsx (which would need to manage layout itself), so the module earns its keep.

### Item 3: "App composes it"

**CLEAN.** App already composes every section. Wrapping them in a layout component is a natural refactoring that preserves the existing data flow. App continues to own all state and event handlers; the layout component only arranges children.

### Item 4: "State: which region holds which existing section is layout-only, not committed"

**CLEAN.** No new durable state. The layout is reconstructed from the component tree on every render. A restart reconstructs everything from SQLite, exactly as today.

### Item 5: "Rail is a fixed-width left column"

**CAUTION — minor.** The PRD specifies a fixed-width rail but does not specify the width or how it behaves on narrow windows (User Story 4). The PRD says "I want the rail to remain usable, so that narrow widths do not break navigation" but provides no implementation guidance for this.

**Resolution:** Choose a reasonable fixed width (e.g., 240px) and add a CSS `min-width` on the rail. For narrow windows (<640px), the rail can collapse to icon-only or use a horizontal scroll. This is a CSS-only decision that does not affect the module structure.

### Item 6: "Main pane is a flex column (top bar region reserved for later slices, scrollable Notes area, footer capture region)"

**CLEAN.** The top bar region is an empty placeholder for future use (R3/R4/R6). It adds no complexity — it is a `<div>` with no children. The scrollable Notes area and footer capture region are clearly separated.

### Item 7: "Visual: Modernist tokens/classes only; 2px --color-divider between regions; zero radius"

**CLEAN.** Pure CSS. No new component logic. The divider is a single border or box-shadow property. Zero radius means no border-radius on the shell regions (the existing section cards keep their own border-radius).

### Item 8: "This slice relocates existing content into the shell; it does not restyle the Note card, move settings, or add the canvas"

**CLEAN.** Explicit scope fence. The PRD names what is NOT changing, which prevents scope creep during implementation.

### Item 9: "Test that, given a snapshot with N Notes in the active Workspace, the main pane renders those N Notes and the rail renders the Workspace list"

**CLEAN.** The existing test seam (mock `invoke`, render `<App />`, assert via DOM queries) is preserved. The tests assert placement (which region contains which content), not styling. This is consistent with the prior art in `graph-view.test.tsx` and `note-views.test.ts`.

### Item 10: "Test that the three regions are present and that resizing does not drop the rail or capture region"

**CAUTION — minor.** Testing "resizing does not drop the rail" in jsdom is limited because jsdom does not implement layout. The test can assert that the three regions are present in the DOM with their expected roles/aria-labels. A true resize test would require a browser environment (Playwright), which is not part of the current test infrastructure.

**Resolution:** Test that the three regions are present with correct aria-labels. Document the jsdom limitation. Do not add Playwright for this slice.

---

## COMPLEXITY SCORECARD

**State Surface:** Low — no new mutable state. The layout is derived from the component tree on every render.

**Seam Quality:** Preserved — the durable seam (`workspace-client.ts`) is untouched. All existing sections keep their interfaces. The new layout component is a pure composition layer.

**Module Cohesion:** Cohesive — the new layout component has one responsibility (arranging three regions). It does not own business logic, state, or event handlers.

**Change Blast Radius:** Narrow — a future change to the layout (e.g., adding a fourth region, changing rail width) edits only the layout component and CSS. No section component or test needs to change.

**Incidental Complexity Load:** Mostly Problem — the complexity is intrinsic to the problem (replacing a single-column layout with a three-region shell). The implementation plan adds no incidental complexity.

**Summary:** This is a textbook simple PRD. It introduces one new module with a single responsibility, preserves every existing seam, adds no durable state, and explicitly fences its scope. The two CAUTION items are minor CSS and testing decisions that do not affect the module structure.

---

## GATE DECISION: PROCEED

No BLOCK items. Two minor CAUTION items (rail width on narrow windows, jsdom resize testing limitation) are documented above and do not require PRD changes. Hand to implementation with confidence.

The implementation plan is structurally sound. Build the layout component, move the existing sections into their regions, add the CSS, and update the tests to assert region placement.
