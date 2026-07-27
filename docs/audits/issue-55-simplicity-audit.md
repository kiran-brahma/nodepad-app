# PRD SIMPLICITY AUDIT

Feature: R14 — Workspace rail & switcher
Date: 2026-07-27
Gate: PROCEED

---

## MODULE MAP

| Module | Current responsibility and interface |
| --- | --- |
| `src/workspace-client.ts` | The sole frontend seam. `createWorkspace(name)`, `selectWorkspace(id)`, `renameWorkspace(id, name)`, `deleteWorkspace(id)` over the Rust command surface. The "always leave one valid Workspace" rule lives behind `deleteWorkspace` in Rust. |
| `src/workspace-section.tsx` | The rail's contents: today a wrapping row of Workspace buttons, an always-visible create form, and the Settings entry in the rail footer. Owns no state and no command; App passes handlers. |
| `src/app-shell.tsx` | Layout only: the `nav[aria-label="Workspaces"]` rail region, top bar, content, footer. |
| `src/workspace-lifecycle.ts` | The delete confirmation: `requestDelete`, `deleteConfirmationPrompt`, `resolveDeleteConfirmation`. Unchanged by this slice. |
| `src/workspace-settings-sheet.tsx` | R3's sheet. Renders the rename form from App's `renameDraft` and the delete confirmation from App's `pendingDelete`. |
| `src/command-palette.tsx` | Renders `PaletteAction[]` and runs the selected one. Knows nothing about Workspaces; App's `buildPaletteActions` decides what exists. |
| `src/App.tsx` | Owns `workspaceName` (create draft), `renameDraft`, `pendingDelete`, and every submit through the seam. |

Durable decisions this slice must not re-litigate: the `thinkingWorkspace` seam, the delete confirmation and the one-valid-Workspace invariant, the `CONTEXT.md` glossary (Thinking Workspace, never "project" or "space").

---

## INTERROGATION FINDINGS

### The rail list and the active marker

**CLEAN.** `workspaces` and `activeWorkspaceId` are already read from the committed snapshot on every render, and the list already marks the active one. Turning a wrapping row into a vertical list with a 2px accent left edge is a change of markup and CSS over the same two inputs. Selecting a row still calls `selectWorkspace`; which Workspace is active remains a fact the snapshot reports, never a fact the rail holds.

### Inline rename in the rail vs. rename in the settings sheet

**CAUTION → resolved by sharing the one draft.** R3's sheet already renames from App's `renameDraft`, and the palette's "Rename Workspace" action already sets it. A second draft owned by the rail would give the application two answers to "what name is being edited," and they would drift the moment either surface changed.

Resolution (revised during code review): a draft is not a fact about the Workspace — it is which field is open and what is typed in it, and that is per surface. Sharing one value made both surfaces render a field at once: two `autoFocus` inputs racing, two escape-stack entries, and the rail row's select button replaced by an input while the sheet was open. The shipped answer keeps two drafts, `railRenameDraft` and the sheet's `renameDraft`, and removes the duplication where it actually was — one `WorkspaceRenameForm` (`src/workspace-rename-form.tsx`, with the `WorkspaceRenameDraft` type) that both surfaces render with their own accessible name. One commit path, `renameWorkspace(draft, clearDraft)`, so neither surface owns a rename rule. The ⌘K "Rename Workspace" action now opens the rail row's field, the surface R14 gives rename, instead of setting a draft with nothing on screen to show it.

### The create control

**CLEAN, with one bounded piece of local state.** The issue asks for a create control that *opens* an inline name field, so whether the field is open is a fact the rail alone knows — it is not derivable from the snapshot and nothing else in the app needs it. Keeping it in `WorkspaceSection` is the narrowest home. The name itself stays App's `workspaceName` draft, cleared by App on commit, so the field's contents remain single-sourced.

For the rail to close the field only on a committed create, `onCreate` returns whether the command committed (`Promise<boolean>`) instead of taking a `FormEvent`. That is a subtraction: App no longer needs to hand the rail a form-event handler, and the rail no longer guesses at commit from a cleared draft. A refused create leaves the field open with the name intact; Escape closes the field and clears the name with it, so reopening never resumes a name the thinker walked away from.

### The ⌘K jump

**CLEAN.** `buildPaletteActions` is already the one place that decides which actions exist, and the palette itself learns nothing new. A jump entry per Workspace is a `map` over the same `workspaces` array the rail renders, running the same `selectWorkspace` the rail row runs. No new state, no new command, no palette-side knowledge of Workspaces. Full palette coverage stays R15's.

### Deletion

**CLEAN — untouched.** Delete keeps its confirmation in the settings sheet and its palette entry, resolved through `workspace-lifecycle.ts`. The one-valid-Workspace invariant is enforced in Rust behind `deleteWorkspace`; the rail adds no deletion path, so there is no second place for the invariant to be restated or bypassed.

### Change test

If the rail's shape, accent, or row actions change in 6 months, `workspace-section.tsx` and `styles.css` change. If renaming ever needs to do more, App's `renameWorkspace` changes and both surfaces follow. If the palette gains commands, `buildPaletteActions` changes and the palette does not.

---

## COMPLEXITY SCORECARD

State Surface: Low — one new boolean (`creating`) local to the rail, and one rename draft per surface. Every other input is committed snapshot state or an App draft that already existed.

Seam Quality: Preserved — four existing commands, no new one, and no durable field added.

Module Cohesion: Cohesive — layout in the shell, presentation in the section, drafts and commits in App, invariants in Rust.

Change Blast Radius: Narrow — `workspace-section.tsx`, `workspace-rename-form.tsx` (new, extracted from the settings sheet), `workspace-settings-sheet.tsx` (shrinks), `App.tsx` (props and palette actions), `styles.css`, plus tests.

Incidental Complexity Load: Mostly Problem — the one avoidable complexity, a second rename *form*, is removed by extracting the one both surfaces render.

Summary: The slice is a new arrangement of state the app already holds, over commands it already has. Nothing durable is added, so nothing durable can drift.

---

## GATE DECISION: PROCEED

Implement as specified, with two recorded structural choices: the in-place rename *form* is extracted once and rendered by both the rail and the settings sheet, each over its own draft (revised from the pre-implementation plan to share one draft — see the finding above), and `onCreate` reports whether the create committed so the rail's inline field closes on a commit rather than inferring it from a cleared draft.
