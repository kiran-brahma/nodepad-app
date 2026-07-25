# PRD SIMPLICITY AUDIT

Feature: R5 — Inline editing everywhere
Issue: kiran-brahma/nodepad-app#46
Date: 2026-07-26
Gate: **PROCEED**

---

## MODULE MAP

### Existing modules the PRD touches

| Module | Current responsibility | What changes |
|--------|----------------------|--------------|
| `src/note-card.tsx` | Renders one Note card with resting layout, action row, ⌘· menu, and draft editors (text, annotation, labels, relate, transfer, delete). | **Thought:** Replace the current `NoteText` component (resting markdown render + separate edit form) with a click-to-edit inline region. Click enters editable mode; blur or Enter commits via `editNoteText`; Shift+Enter inserts newline; Escape reverts. |
| `src/note-card.tsx` | `NoteText` component renders markdown when not editing, a `<textarea>` + save/cancel buttons when editing. | **Annotation:** Replace the current `NoteAnnotation` component (resting annotation display + separate edit form) with click-to-edit inline. Click enters edit; blur or Enter commits via `setNoteAnnotation`; Escape reverts. Enforce `MAX_ANNOTATION_SCALARS` (2000) limit. |
| `src/note-card.tsx` | `ActionRow` has an "Edit text" button. | **Type:** Replace the current Type tag + ⌘· menu select with a click-on-tag popover listing all 14 `NOTE_TYPES`. Choosing one calls `setNoteType` and closes. |
| `src/note-card.tsx` | `CommandMenu` has a "Set Type" select. | Remove the "Edit text" button from `ActionRow` (editing is now click-on-thought). Remove "Set Type" and "Edit Annotation"/"Add Annotation" from `CommandMenu` (editing is now inline). |
| `src/note-intents.ts` | Builds the one set of `NoteIntents` for all views. | Unchanged — `editNoteText`, `setNoteType`, `setNoteAnnotation` are the same commands. The `NoteIntents` interface is unchanged. |
| `src/note-drafts.ts` | `NoteDrafts` holds per-Note draft state. | Unchanged — inline editing uses the same `noteDraft` and `annotationDraft` state. |
| `src/note-controls.ts` | `MAX_ANNOTATION_SCALARS`, `isAnnotationTooLong`, `annotationLength`. | Unchanged — annotation length enforcement is reused as-is. |

### Existing modules the PRD does NOT touch

- `src/workspace-client.ts` — unchanged (durable seam, explicitly preserved)
- `src/App.tsx` — unchanged (no new state, no new wiring)
- `src/note-intents.ts` — unchanged (same commands, same interface)
- `src/note-drafts.ts` — unchanged (same draft state)
- `src/note-controls.ts` — unchanged (same validation)
- `src/note-views.ts` — unchanged
- `src/note-views.test.ts` — unchanged
- `src/note-controls.test.ts` — unchanged
- `src/App.test.tsx` — unchanged (existing tests continue to pass; new tests in a new file)
- `src/thinking-graph.ts` — unchanged
- `src/note-transfer.ts` — unchanged
- `src/note-focus.ts` — unchanged
- `src/enrichment-controller.ts` — unchanged (the `enrichmentRevision` guard already invalidates in-flight AI responses on edit)
- All other modules — unchanged

### New modules

- **`src/note-card.test.tsx`** — tests for the inline editing behavior at the existing seam (mock `invoke`, render `<App />`, assert via DOM queries). Tests:
  - Clicking a thought enters inline edit mode
  - Blurring commits via `editNoteText`
  - Enter commits via `editNoteText`
  - Shift+Enter inserts newline
  - Escape reverts without a call
  - Type popover lists all 14 types
  - Selecting a type calls `setNoteType`
  - Annotation click-to-edit
  - Annotation length enforcement
  - Empty commit is rejected

---

## INTERROGATION FINDINGS

### Item 1: "Click the thought to edit it in place"

**CLEAN.** The current `NoteText` component already has a resting state (markdown render) and an editing state (textarea + save/cancel buttons). The change is to:
1. Make the resting markdown clickable (add `onClick` that calls `intents.startEdit`)
2. Replace the save/cancel buttons with blur + Enter commit behavior
3. Add Shift+Enter for newline
4. Add Escape to revert

The `NoteIntents` interface already has `startEdit`, `editTextDraft`, `saveText`, and `cancelEdit`. No new intents needed.

The `NoteDrafts` already has `noteDraft` state. No new draft state needed.

### Item 2: "Change the Note Type from an inline popover on the Type tag"

**CLEAN.** The current Type tag is a `<span className="tag">` in the resting layout. The change is to make it clickable, opening a small popover listing all 14 `NOTE_TYPES`. Choosing one calls `intents.setNoteType` and closes.

The `NoteIntents` already has `setNoteType`. No new intents needed.

The popover is a small transient UI element (not a modal), so it doesn't need focus trapping or Escape handling beyond the popover's own close-on-select behavior.

### Item 3: "Edit the Annotation in place"

**CLEAN.** Similar to the thought: the current `NoteAnnotation` component already has a resting state and an editing state. The change is to:
1. Make the resting annotation clickable (or the "Add annotation" affordance)
2. Replace save/cancel buttons with blur + Enter commit
3. Enforce `MAX_ANNOTATION_SCALARS` (2000) limit inline
4. Empty saves clear the Annotation

The `NoteIntents` already has `startAnnotation`, `editAnnotationDraft`, `saveAnnotation`, and `cancelAnnotation`. No new intents needed.

The `NoteDrafts` already has `annotationDraft` state. No new draft state needed.

### Item 4: "Enter/blur to save and Escape to cancel"

**CLEAN.** This is a behavior change in the `NoteText` and `NoteAnnotation` components. The current pattern uses explicit Save/Cancel buttons. The new pattern uses:
- `onBlur` on the textarea → calls `intents.saveText` / `intents.saveAnnotation`
- `onKeyDown` with Enter (no Shift) → calls save
- `onKeyDown` with Escape → calls cancel
- `onKeyDown` with Shift+Enter → inserts newline (native textarea behavior)

The `EscapeDismiss` component is already used in the current edit forms. The new pattern removes the explicit buttons and uses the textarea's native behavior.

### Item 5: "Shift+Enter to add a line inside a thought"

**CLEAN.** This is native `<textarea>` behavior. The textarea already handles Shift+Enter as a newline. The change is to intercept plain Enter (without Shift) to commit, while letting Shift+Enter pass through.

### Item 6: "Annotation length limit enforced inline"

**CLEAN.** The existing `isAnnotationTooLong` and `annotationLength` functions from `note-controls.ts` are reused. The current annotation editor already shows the character count and disables the save button when over limit. The inline version keeps this behavior but without the save button — instead, it prevents blur/Enter from committing when over limit.

### Item 7: "Drafts use transient per-Note draft state"

**CLEAN.** The existing `useNoteDrafts` hook and `NoteDrafts` interface are reused unchanged. No new draft state.

### Item 8: "Empty commit is rejected"

**CLEAN.** The current `saveText` in `note-intents.ts` already checks `if (!draft) return`. The inline version adds a check for empty/whitespace markdown before committing, matching the current validation in `App.tsx`'s `createNote` (`if (noteMarkdown.trim() === "") return`).

### Item 9: "Testing at the existing seam"

**CLEAN.** The existing test infrastructure (mock `invoke`, render `<App />`, assert via DOM queries) is preserved. New tests go in a new `src/note-card.test.tsx` file, mirroring the existing pattern.

### Item 10: "The enrichment revision guard already invalidates in-flight AI responses on edit"

**CLEAN.** The `enrichmentRevision` field on `Note` is bumped on every commit that touches the Note. The Enrichment Workflow captures it into the request token and rejects any response that names a different revision. This slice relies on that existing guard and adds no new guard.

---

## COMPLEXITY SCORECARD

**State Surface:** Low — no new state. The existing `NoteDrafts` (noteDraft, annotationDraft) are reused. The Type popover is a small local `useState` in `NoteCard` (like the existing `commandMenuOpen`).

**Seam Quality:** Preserved — the durable seam (`workspace-client.ts`) is untouched. All commands (`editNoteText`, `setNoteType`, `setNoteAnnotation`) are reused unchanged. The `NoteIntents` interface is unchanged.

**Module Cohesion:** Cohesive — all changes are in `src/note-card.tsx`. The component already has the resting/editing dual state; the change is to the interaction pattern (click-to-edit instead of button-to-edit). No new module is needed.

**Change Blast Radius:** Narrow — changes are limited to:
1. Modified `src/note-card.tsx` (inline editing behavior, Type popover, removed action row items)
2. New `src/note-card.test.tsx` (tests for inline editing)

A future change to editing behavior edits only `note-card.tsx`. A future change to the Type list edits only `workspace-client.ts` (the `NOTE_TYPES` constant).

**Incidental Complexity Load:** None — the complexity is intrinsic to the problem (making reading and editing one view). The implementation adds no incidental complexity. Every pattern (click-to-edit, blur-to-save, Escape-to-cancel) is already established in the codebase (the capture bar uses Enter to commit, the search field uses Enter to submit).

---

## GATE DECISION: PROCEED

No BLOCK items. No CAUTION items. Pure interaction pattern change in one component, reusing existing intents, drafts, and commands. Hand to implementation with confidence.

The implementation plan is structurally sound:
1. Modify `NoteText` in `note-card.tsx` to support click-to-edit with blur/Enter commit and Escape cancel
2. Modify `NoteAnnotation` in `note-card.tsx` to support click-to-edit with blur/Enter commit and Escape cancel
3. Add Type popover to the Type tag in `note-card.tsx`
4. Remove "Edit text" from `ActionRow`, remove "Set Type" and "Edit Annotation"/"Add Annotation" from `CommandMenu`
5. Add tests in `src/note-card.test.tsx`
