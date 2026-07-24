# PRD SIMPLICITY AUDIT

Feature: R3 — Workspace settings out of the main view
Issue: kiran-brahma/nodepad-app#44
Date: 2026-07-25
Gate: **PROCEED**

---

## MODULE MAP

### Existing modules the PRD touches

| Module | Current responsibility | What changes |
|--------|----------------------|--------------|
| `src/App.tsx` | V0 orchestrator. Renders `AssistanceSection` and `CaptureSection` (workspace admin) in the main pane. | Removes `AssistanceSection` from main pane. Removes workspace admin controls from `CaptureSection` rendering. Adds settings sheet state and wiring. Adds Settings button to rail. |
| `src/capture-section.tsx` | Renders workspace admin controls (rename, delete, export, import) and workspace name heading. | Loses the admin controls. Becomes a simple workspace name heading (or is removed entirely if the name moves to the rail). |
| `src/workspace-section.tsx` | Renders workspace list and create form in the rail. | Adds a Settings button in the rail footer. |

### Existing modules the PRD does NOT touch

- `src/workspace-client.ts` — unchanged (durable seam, explicitly preserved)
- `src/assistance-section.tsx` — moved verbatim into settings sheet, unchanged
- `src/cloud-key-section.tsx` — moved verbatim into settings sheet, unchanged
- `src/cloud-consent-dialog.tsx` — unchanged
- `src/cloud-provider.ts` — unchanged
- `src/use-local-discovery.ts` — unchanged
- `src/use-cloud-discovery.ts` — unchanged
- `src/escape-stack.ts` — unchanged
- `src/modal-focus.ts` — unchanged
- `src/escape-dismiss.tsx` — unchanged
- `src/workspace-lifecycle.ts` — unchanged
- `src/committed-notes-section.tsx` — unchanged
- `src/search-section.tsx` — unchanged
- `src/synthesis-section.tsx` — unchanged
- `src/capture-bar.tsx` — unchanged
- `src/app-shell.tsx` — unchanged
- All controllers, hooks, and test infrastructure — unchanged

### New modules

- **`src/workspace-settings-sheet.tsx`** — a modal dialog (`.dialog` over `.dialog-backdrop`) that hosts the existing `AssistanceSection`, `CloudKeySection`, export/import buttons, and rename/delete controls. Focus-trapped like `RenameLabelModal`. Escape and scrim click close it.

---

## INTERROGATION FINDINGS

### Item 1: "Move Assistance Policy, model discovery, cloud key/consent, export/import, and rename/delete into a settings sheet"

**CLEAN.** This is pure relocation. Every control keeps its exact current behavior. The `AssistanceSection` component is imported and rendered inside the sheet with the same props. The `CloudKeySection` is rendered inside the sheet. The export/import/rename/delete buttons are rendered inside the sheet with the same handlers.

The settings sheet is a modal dialog, focus-trapped like the existing `RenameLabelModal`. This pattern is well-established in the codebase.

### Item 2: "Settings entry in the rail"

**CLEAN.** A single button in the rail footer. Opens the settings sheet for the active Workspace. Disabled when there is no active Workspace.

### Item 3: "Main pane shows only Notes and capture"

**CLEAN.** Removing `AssistanceSection` and the workspace admin controls from the main pane. The main pane then shows: header (Nodepad branding), search, committed notes, synthesis. The capture bar remains in the footer.

### Item 4: "Escape and scrim click close the sheet"

**CLEAN.** Reuses `useModalFocus` (focus trap + restore to invoker) and `useEscape` at `ESCAPE_PRIORITY.modal`. Scrim mousedown on the backdrop closes the sheet. This is the exact same pattern as `RenameLabelModal` and `CloudConsentDialog`.

### Item 5: "Every setting behaves exactly as before"

**CLEAN.** The settings sheet receives the same props and calls the same handlers as the current inline rendering. No logic changes. The `AssistanceSection` component is imported and used identically. The `CloudKeySection` component is imported and used identically. The export/import/rename/delete buttons call the same `App` handlers.

### Item 6: "Cloud consent disclosure flow preserved"

**CLEAN.** The `CloudConsentDialog` is already a separate modal. It opens from the settings sheet the same way it opens from the current inline rendering. The `setAssistancePolicy` handler in `App` is unchanged.

### Item 7: "Testing Decisions — test at the existing seam"

**CLEAN.** The existing test infrastructure (mock `invoke`, render `<App />`, assert via DOM queries) is preserved. Tests assert:
- Opening the sheet renders the policy switch, model discovery, export/import, and rename/delete
- Escape and backdrop click close the sheet and restore focus to the trigger
- A settings action still calls the same command it does today

---

## COMPLEXITY SCORECARD

**State Surface:** Low — one new boolean state (`settingsOpen`) in `App`. The settings sheet is a controlled modal: open/close only. No new mutable state.

**Seam Quality:** Preserved — the durable seam (`workspace-client.ts`) is untouched. All handlers in `App` are reused unchanged. The `AssistanceSection` and `CloudKeySection` components are imported and used identically.

**Module Cohesion:** Cohesive — the new `WorkspaceSettingsSheet` has one responsibility: host workspace settings in a modal dialog. It composes existing components and adds no new business logic.

**Change Blast Radius:** Narrow — changes are limited to:
1. New `WorkspaceSettingsSheet` component
2. Modified `App.tsx` (wiring changes, removed inline sections)
3. Modified `workspace-section.tsx` (added Settings button)
4. Modified `capture-section.tsx` (removed admin controls)
5. Updated tests

A future change to settings content edits only `WorkspaceSettingsSheet`. A future change to the rail layout edits only `workspace-section.tsx`.

**Incidental Complexity Load:** None — the complexity is intrinsic to the problem (relocating settings into a modal). The implementation adds no incidental complexity. Every pattern (modal focus, escape, scrim dismiss) is already established in the codebase.

---

## GATE DECISION: PROCEED

No BLOCK items. No CAUTION items. Pure relocation of existing controls into a modal dialog. Hand to implementation with confidence.

The implementation plan is structurally sound. Create the `WorkspaceSettingsSheet` component, add a Settings button to the rail, remove the settings sections from the main pane, and update tests.
