# PRD SIMPLICITY AUDIT

Feature: R15 — Command palette expansion
Date: 2026-07-27
Gate: PROCEED

---

## MODULE MAP

| Module | Current responsibility and interface |
| --- | --- |
| `src/command-palette.tsx` | Renders `PaletteAction[]` (id, label, group, disabled, run) grouped by heading, runs the selected one, and closes. Owns `useCommandPaletteShortcut`. Knows nothing about Workspaces, views, Notes, or assistance policy. |
| `buildPaletteActions` (was in `src/App.tsx`) | Decides which actions exist, from handlers App already has. The one place palette branching lives. |
| `src/note-intents.ts` | `buildNoteIntents` returns the one `NoteIntents` object every card is handed: `startEdit`, `startRelate`, `togglePinned`, `requestDelete`, `setNoteType`, and the rest. What may be done to a Note is decided here and nowhere else. |
| `src/note-focus.ts` | `useNoteFocus` — which Note the thinker is reading (`focusedNoteId`), what it lights, and Escape to let go. Transient; never committed. |
| `src/capture-bar.tsx` | The pinned capture field, `id="capture-bar"`, `aria-label="New Note"`. Enter commits, Escape blurs. |
| `src/note-views.ts` | `NOTE_VIEWS`, `noteViewLabel`. The one list of arrangements. |
| `src/workspace-client.ts` | The sole frontend seam, including `NOTE_TYPES` and `setNoteType`. |
| `src/App.tsx` | Owns `settingsOpen`, `railRenameDraft`, `pendingDelete`, the view choice, and every submit through the seam. |

Durable decisions this slice must not re-litigate: the `thinkingWorkspace` seam, the `PaletteAction` shape, the ⌘K shortcut, the card's delete confirmation, and the `CONTEXT.md` glossary.

---

## INTERROGATION FINDINGS

### View, Workspace-switch, and assistance actions

**CLEAN — already built, now derived.** These entries existed before this slice. The only change is that the three view entries are a `map` over `NOTE_VIEWS` with `noteViewLabel` instead of three hand-written literals, so a fourth arrangement cannot ship with a palette that has not heard of it. No new state, no new command.

### Focus the capture bar

**BLOCK found, fixed.** The existing "New Note" action ran `document.getElementById("note")?.focus()`. No element carries `id="note"` since R2 replaced the create form with the capture bar (`id="capture-bar"`), so the action was a silent no-op — the palette held a stale fact about a surface it does not own. Reaching for an element id at all is a small complecting of the palette's action list with the capture bar's DOM, but it is the same coupling the codebase already accepts for "focus this field," and routing it through a ref would mean threading a ref from App into a builder that otherwise takes plain handlers. Kept as one `getElementById`, in App where the surfaces meet, not in the palette module.

### Open Workspace settings

**CLEAN.** `settingsOpen` already exists and the rail already sets it. The palette entry is a second caller of the same setter, not a second answer to whether the sheet is open.

### Focused-Note actions

**CLEAN.** The risk here was the palette learning Note rules — deciding what "pin" means, or deleting without the confirmation the card requires. It is avoided by passing the same `noteIntents` object the card is handed: `startEdit`, `startRelate`, `togglePinned`, `requestDelete`, `setNoteType`. `requestDelete` opens the card's own confirmation, so ⌘K cannot destroy a thought the card would have asked about. The palette adds no state: the focused Note is looked up from `visible` by `focus.focusedNoteId`, and focus already lets go of a Note that leaves the screen.

Pin's label reads the Note (`Unpin focused Note` when pinned), which is display derived from committed state, not a second record of it.

### Set Note Type

**CAUTION → resolved by enumeration.** "Set Type" could mean an entry that opens the card's type control (a second surface, and one the palette would have to know how to open) or one entry per Note Type. The second is chosen: fourteen `map`ped entries over `NOTE_TYPES`, in their own group, each running `intents.setNoteType`. It costs list length and buys a keyboard path that needs no follow-up surface — the same shape as R14's one-entry-per-Workspace jump. `NOTE_TYPES` stays the one list; adding a type adds its palette entry for free.

### Disabled while nothing is focused

**CLEAN.** The entries are listed either way and disabled when `focusedNote` is undefined, so the thinker learns they exist rather than wondering where they went. `PaletteAction.disabled` already exists and the palette already refuses to run a disabled action.

### Module ownership of the builder

**CAUTION → resolved by extraction.** `buildPaletteActions` grew from 14 entries to roughly 35 inside `App.tsx`, a file already carrying a `fallow-ignore` for complexity. It moved to `src/palette-actions.ts` unchanged in kind: same inputs, same `PaletteAction[]` output, still a module-level builder so branching stays out of the component body. Deletion test: delete the module and the branching spreads into `App`'s body or into the palette component, which is exactly where it must not be. It earns its keep.

### Change test

If a new action is added, `palette-actions.ts` changes and the palette component does not. If what an action *means* changes, `note-intents.ts` or the App handler changes and the palette follows. If a Note Type or an arrangement is added, neither file changes.

---

## COMPLEXITY SCORECARD

State Surface: Low — no new state. Every input is committed snapshot state, existing transient focus, or an App setter that already had a caller.

Seam Quality: Preserved — no new command, no change to `PaletteAction`, no new test seam. Note actions run through the one intents object.

Module Cohesion: Cohesive — rendering in `command-palette.tsx`, the action list in `palette-actions.ts`, Note meaning in `note-intents.ts`, durable rules in Rust.

Change Blast Radius: Narrow — `palette-actions.ts` (new), `App.tsx` (shrinks), `App.test.tsx`.

Incidental Complexity Load: Mostly Problem — one avoidable tangle (a growing builder inside the orchestrator) removed by extraction; one accepted (`getElementById` for focus), bounded to a single line in App.

Summary: Coverage over surfaces that already exist. The palette gains entries and no knowledge; every new entry ends in a handler or an intent that another surface already calls, so no action can mean two things.

---

## GATE DECISION: PROCEED

Implement as specified, with three recorded structural choices: the builder moves to its own module, Set Note Type is enumerated per `NOTE_TYPES` rather than opening a second surface, and focused-Note entries route through the card's `NoteIntents` — including delete, which keeps its confirmation.
