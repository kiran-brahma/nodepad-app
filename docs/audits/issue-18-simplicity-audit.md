# PRD SIMPLICITY AUDIT

Feature: V0-17 — Complete macOS keyboard, accessibility, and external links
Issue: kiran-brahma/nodepad-app#18
Date: today
Gate: **PROCEED**

---

## MODULE MAP

### Existing modules (will be extended)

- `src/App.tsx` — root. Owns global shortcut wiring (undo), and the inline
  Rename Label `role="dialog"` and the Cloud consent disclosure entry. Wires
  every section to the one `workspace-client` seam.
- `src/undo-shortcut.ts` — owns Command-Z. Already skips text editing so the
  field's own undo wins; no change needed to its contract.
- `src/note-focus.ts` — owns note focus/lock and today clears both on a
  standalone global Escape listener. Will delegate Escape to the new
  coordinator instead of owning it directly.
- `src/note-card.tsx` — owns Note rendering, including `ReactMarkdown`. The
  only place a Note's Markdown can render an `<a>`, so the only place an
  external link can leave the webview. Also owns the Enrichment badge and the
  Annotation over-limit counter.
- `src/cloud-consent-dialog.tsx` — a real modal (`role="dialog"`). Today it
  has no focus trap or focus restoration.
- `src/capture-section.tsx` — the inline Workspace rename form and the
  delete-confirm `role="alertdialog"`.
- `src/synthesis-section.tsx` — the one Synthesis status sentence.
- `src/styles.css` — global presentation. Already gives graph marks a
  `:focus-visible` indicator; no global focus-visible or reduced-motion rule.
- `src/workspace-client.ts` — the UI's only durable-state seam; every Tauri
  command binding lives here.
- `src-tauri/src/lib.rs` — the Tauri command surface and `AppState`. Dialog
  is invoked from Rust (`DialogExt`), so the project ships with no
  `capabilities/` directory and no frontend permission files.

### New modules

- `src-tauri/src/external_link.rs` — owns external-link scheme validation and
  the macOS shell opener. A pure `is_openable_external_url(&str) -> bool`
  carries every scheme decision (V0 opens `http`/`https` only); the command
  body only invokes `open` after that predicate passes. This is where
  "the webview never navigates and only HTTP(S) leaves Nodepad" lives, once.
- `src/escape-stack.ts` — owns coordinated Escape. A priority-ordered stack
  of close callbacks; the one global listener closes the topmost dismissible
  surface, and only the lowest priority falls through to clearing focus.
- `src/modal-focus.ts` — owns focus trap + restore-to-invoker for true
  modals. One `useModalFocus(open)` hook; modals opt in, the rest of the UI
  keeps its inline form style.
- `src/command-palette.tsx` — owns the Command-K palette. Renders a list of
  actions; App decides which actions exist and what each does, so the palette
  never learns a business rule.
- `src/external-link.tsx` — the ReactMarkdown `a` override. Routes openable
  URLs through the one seam and renders unsupported schemes as inert text, so
  no anchor in a Note can ever navigate the webview.

---

## INTERROGATION FINDINGS

### Coordinated Escape across every dismissible surface

**CAUTION → resolved.** The naive reading — "each surface listens for its
own Escape" — complects ordering with React effect order: child effects run
before parent effects, so a modal mounted later would register *under* the
always-mounted note-focus listener and Escape would clear focus instead of
closing the modal. Resolved with an explicit-priority escape stack: modals
register high, inline drafts register medium, note-focus registers lowest
and is never popped. The coordinator owns "which surface is topmost"; no
surface knows about any other. **CLEAN after resolution.**

### Focus trap and restore-to-invoker for modals

**CLEAN.** A single `useModalFocus(open)` hook saves `activeElement` on open,
moves focus into the dialog, cycles Tab within it, and restores focus on
close. Applied to the Cloud consent dialog, the Rename Label dialog, and the
command palette — the three true modals. Inline note edits and the workspace
rename form keep their inline style; they are not modals and do not trap.

### Command palette (Command-K)

**CLEAN.** The palette takes an `actions: PaletteAction[]` list and renders
it. App builds the list from handlers that already exist (new note, undo,
views, export, archive, rename/delete workspace, assistance policy), so the
palette introduces no new business rule and no new durable state beyond an
`open` boolean. Command-K is not a text-editing shortcut, so intercepting it
globally does not steal from text editing.

### External links leave through the macOS shell opener after scheme validation

**CLEAN.** Scheme validation is a pure, unit-tested predicate in
`external_link.rs`; the command body runs `open` only after it passes. V0
rejects every non-HTTP(S) scheme, including `mailto`, `tel`, `file`, and
`javascript`. The frontend override `preventDefault`s every anchor click and
routes through the one seam, so the webview never navigates; unsupported
schemes render as inert `<span>` text and cannot be opened at all. Using the
`open` binary keeps the project's no-capabilities/no-frontend-permission
setup intact and avoids coupling to a new Tauri plugin's scope config.

### Reduced motion preserves meaning without animation

**CLEAN.** The V0 stylesheet has no keyframe animations or transitions today,
and `framer-motion` is a dependency that the V0 surfaces do not import, so
there is nothing essential to suppress. A defensive `prefers-reduced-motion`
media query neutralises any future transition and documents the intent; no
state is hidden by it.

### Status and errors use text/semantics, not color alone

**CLEAN.** The failure `aside` (`role="alert"`), the Synthesis stale notice
(`role="status"`), and the Enrichment badges already carry text. Two gaps:
the Annotation over-limit counter signals "too long" by color only, and the
Synthesis status sentence is not announced as it changes. Both are closed
with text/semantics — the counter reads "Over the limit" when exceeded, and
the status sentence gets `aria-live="polite"`.

---

## COMPLEXITY SCORECARD

State Surface: **Low** — one `open` boolean for the palette; everything else
is transient UI state already owned by its existing module.
Seam Quality: **Preserved** — `workspace-client` stays the only durable seam;
external links add one binding there; Escape and focus are new pure-UI seams.
Module Cohesion: **Cohesive** — each new module has one responsibility
(scheme+opener, Escape priority, focus trap, palette rendering, link override).
Change Blast Radius: **Narrow** — a new scheme rule changes one predicate; a
new dismissible surface registers one `useEscape`; a new modal opts into one
hook.
Incidental Complexity Load: **Mostly Problem** — coordinating Escape and
trapping focus are intrinsic to a multi-surface macOS app, not introduced by
the implementation.

Summary: The PRD is structurally sound. One caution (Escape ordering across
parent/child effects) is resolved by an explicit-priority coordinator so no
surface knows about another. No existing clean seam is crossed; the
durable-state seam gains exactly one binding.

---

## GATE DECISION: PROCEED

Hand `external_link.rs`, the `open_external_link` command + seam binding,
`escape-stack.ts`, `modal-focus.ts`, `command-palette.tsx`, the
`external-link.tsx` override, the modal/dialog wiring, the focus-visible and
reduced-motion CSS, and the status text/semantics fixes to implementation.