# PRD SIMPLICITY AUDIT

Feature: R2 — Persistent capture bar
Issue: kiran-brahma/nodepad-app#43
Date: 2026-07-25
Gate: **PROCEED**

---

## MODULE MAP

### Existing modules the PRD touches

| Module | Current responsibility | What changes |
|--------|----------------------|--------------|
| `src/App.tsx` | V0 orchestrator. Owns all state, event handlers, section wiring. Renders `CaptureSection` in the footer with workspace admin buttons + note capture form. | Replaces `CaptureSection` in footer with new `CaptureBar`. Moves workspace admin controls to top bar. Wires new capture bar handlers. |
| `src/capture-section.tsx` | Renders workspace controls (rename, delete, export, import) and the note capture form (textarea + submit button). | Removes the note capture form. Becomes a workspace admin section rendered in the top bar. |
| `src/committed-notes-section.tsx` | Renders committed Notes in the chosen view. Shows "No Notes yet." when empty. | Shows a first-Note prompt (heading + body) when the active Workspace has zero Notes. |
| `src/styles.css` | All styles. Has shell layout, section cards, views. | Adds capture bar styles (single-line growing input, disabled state). |

### Existing modules the PRD does NOT touch

- `src/workspace-client.ts` — unchanged (durable seam, explicitly preserved)
- `src/workspace-section.tsx` — unchanged
- `src/assistance-section.tsx` — unchanged
- `src/search-section.tsx` — unchanged
- `src/synthesis-section.tsx` — unchanged
- `src/note-card.tsx` — unchanged
- `src/note-views.ts` — unchanged
- `src/note-intents.ts` — unchanged
- `src/note-drafts.ts` — unchanged
- `src/note-focus.ts` — unchanged
- `src/thinking-graph.ts` — unchanged
- `src/graph-view.tsx` — unchanged
- `src/tiling-view.tsx` — unchanged
- `src/kanban-view.tsx` — unchanged
- `src/app-shell.tsx` — unchanged (layout already has three regions)
- All controllers, hooks, and test infrastructure — unchanged

### New modules

- **`src/capture-bar.tsx`** — a single-line growing textarea/input pinned to the footer. Enter commits (non-empty), Shift+Enter inserts newline, empty Enter no-op, Escape blurs. Disabled with placeholder when no active workspace.

---

## INTERROGATION FINDINGS

### Item 1: "Move Note capture into an always-present single-line-growing bar at the foot of the main pane"

**CLEAN.** The footer region already exists in the R1 shell (`app-main-footer`). This is a pure relocation: the capture form moves from inside `CaptureSection` (which also held workspace admin controls) into a dedicated `CaptureBar` component. The existing `createNote` handler in `App` is reused unchanged.

The workspace admin buttons (rename, delete, export, import) currently share `CaptureSection` with the capture form. Moving capture out means the admin buttons need a new home. The natural place is the `app-main-topbar` region (already reserved in R1 for later slices). This is a layout-only change — no new state, no new handlers.

### Item 2: "Enter commits the Note via createNote; the pane keeps its scroll position and the input refocuses for the next thought"

**CLEAN.** The `createNote` handler already exists in `App` and is unchanged. Scroll position preservation is a natural consequence of React's reconciliation: the Notes area (`app-main-content`) is a separate scroll container from the footer (`app-main-footer`), so updating the snapshot does not reset its scroll position. The input refocus is a `useEffect` on the capture bar component that re-focuses after commit.

**CAUTION — minor.** The existing `createNote` handler in `App` uses `FormEvent` (from a `<form onSubmit>`). The new capture bar uses a keyboard event (Enter key). The handler signature needs to change from `(event: FormEvent) => void` to `() => void` (or the bar wraps in a form). Using a `<form>` with `onSubmit` is simpler and preserves the existing handler signature.

**Resolution:** Wrap the capture bar in a `<form>` with `onSubmit`, matching the existing pattern. This keeps the handler interface unchanged.

### Item 3: "Shift+Enter to insert a newline"

**CLEAN.** This is a standard textarea keydown handler. The capture bar is a `<textarea>` (not `<input>`) to support multi-line input. Enter without Shift calls `event.preventDefault()` and submits; Shift+Enter inserts a newline naturally.

### Item 4: "Empty Workspace shows a first-Note prompt"

**CLEAN.** The `CommittedNotesSection` already handles the empty state ("No Notes yet."). This replaces that text with a richer prompt. The condition is the same (`notes.length === 0`). No new state. The prompt is static text — no interaction, no dismissal.

### Item 5: "Scroll position preserved after committing"

**CLEAN.** The R1 shell already separates the scrollable content area (`app-main-content`, `overflow-y: auto`) from the pinned footer (`app-main-footer`, `flex-shrink: 0`). A snapshot update re-renders the Notes list inside the scrollable area without resetting its scroll position. No special handling needed.

### Item 6: "Disabled: the bar is disabled with placeholder when there is no active Workspace"

**CLEAN.** The capture bar receives `activeWorkspace` as a prop. When undefined, the textarea is `disabled` and the placeholder changes. This is a pure presentational concern.

### Item 7: "Interface: unchanged — capture calls thinkingWorkspace.createNote(workspaceId, markdown)"

**CLEAN.** The durable seam is untouched. The `createNote` handler in `App` is reused as-is, including the post-commit enrichment scheduling.

### Item 8: "Testing Decisions — test at the existing seam"

**CLEAN.** The existing test infrastructure (mock `invoke`, render `<App />`, assert via DOM queries) is preserved. The tests assert:
- Committing a Note calls `createNote` with the correct args and clears the draft
- Enter vs Shift+Enter behavior and empty-Enter no-op
- Empty Workspace renders the first-Note prompt
- Workspace with Notes does not show the prompt

These are all DOM-level assertions using the existing seam. No new test infrastructure needed.

### Item 9: "Out of Scope — AI auto-typing, markdown live-preview, type auto-detect chip"

**CLEAN.** Explicit scope fence. Prevents scope creep during implementation.

---

## COMPLEXITY SCORECARD

**State Surface:** Low — no new mutable state. The capture draft is already managed in `App` as `noteMarkdown`. The new `CaptureBar` component is controlled (receives value + onChange from parent). The empty-state prompt is derived from existing data (`notes.length === 0`).

**Seam Quality:** Preserved — the durable seam (`workspace-client.ts`) is untouched. The `createNote` handler in `App` is reused unchanged. The new `CaptureBar` component is a pure presentational component with no business logic.

**Module Cohesion:** Cohesive — the new `CaptureBar` has one responsibility: capture input with keyboard handling. The existing `CaptureSection` loses the capture form and becomes a workspace admin section (rename, delete, export, import) — this is a natural separation of concerns.

**Change Blast Radius:** Narrow — changes are limited to:
1. New `CaptureBar` component (can be modified independently)
2. Modified `CaptureSection` (loses capture form)
3. Modified `CommittedNotesSection` (richer empty state)
4. Modified `App.tsx` (wiring changes)
5. CSS additions

A future change to capture behavior (e.g., adding type auto-detect chip) edits only `CaptureBar`. A future change to empty state edits only `CommittedNotesSection`.

**Incidental Complexity Load:** Mostly Problem — the complexity is intrinsic to the problem (relocating capture, adding keyboard handling, improving empty state). The implementation adds no incidental complexity.

**Summary:** This is a clean, well-scoped PRD. It introduces one new module with a single responsibility, preserves every existing seam, adds no durable state, and explicitly fences its scope. The workspace admin buttons naturally separate from the capture form as a side effect of the relocation. The one CAUTION item (handler signature) has a straightforward resolution.

---

## GATE DECISION: PROCEED

No BLOCK items. One minor CAUTION item (handler signature) is resolved by wrapping the capture bar in a `<form>` to preserve the existing `onSubmit` pattern. Hand to implementation with confidence.

The implementation plan is structurally sound. Build the `CaptureBar` component, remove the capture form from `CaptureSection`, move workspace admin controls to the top bar, add the empty-state prompt to `CommittedNotesSection`, add CSS, and update tests.
