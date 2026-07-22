## Parent

Part of #1.

## What to build

Complete macOS keyboard, accessibility, focus, reduced-motion, and external-link behavior across the finished V0 surfaces. This is a user-visible interaction slice, not a broad redesign.

## Decisions

- Required shortcuts: Return submits capture when appropriate; Command-K opens command palette; Command-Z invokes session undo; Escape clears focus/lock or closes the topmost dismissible surface.
- Do not steal shortcuts from text editing, native dialogs, or assistive technology.
- Every interactive action is keyboard reachable with visible focus.
- Modals trap focus, restore it to the invoker, label controls semantically, and dismiss only under defined rules.
- Status and errors use text/semantics, not color alone.
- Respect `prefers-reduced-motion`; preserve meaning without animation.
- External HTTP(S) links open through the Tauri/macOS shell opener after scheme validation. Nodepad's webview never navigates away.
- Reject non-HTTP(S) external schemes in V0.

## Acceptance criteria

- [ ] Capture, command palette, undo, and Escape work consistently across tiling, kanban, graph, settings, search, Note detail, Synthesis, and export surfaces.
- [ ] Text editing retains expected macOS editing behavior.
- [ ] Keyboard-only users can create/organize/search/navigate/export without a focus trap.
- [ ] Modals have correct focus entry, cycling, labeling, dismissal, and restoration.
- [ ] Screen-reader names and live status make provider, save, error, and pending states understandable.
- [ ] Reduced-motion mode removes nonessential animation without hiding transitions or state.
- [ ] Valid external links open outside Nodepad; invalid schemes are rejected; the app view remains intact.
- [ ] Changes remain scoped to V0 surfaces touched by predecessor issues.

## Testing decisions

- Add user-level keyboard and accessibility integration tests; avoid asserting CSS implementation.
- Run automated accessibility checks plus a documented manual VoiceOver smoke pass where automation cannot prove behavior.
- Test shortcut conflicts, nested surfaces, focus restoration, reduced motion, valid/invalid external links, and webview non-navigation.

## Blocked by

- #8
- #9
- #11
- #14
- #15

## Scope fence

Do not redesign visual identity, add customizable shortcuts, support non-macOS key maps, or refactor untouched legacy UI for aesthetics.

## Delivery workflow

Run `prd-simplicity-audit`, use a fresh `implement` session, then `code-review`, scoped `fallow`, focused tests and repository gates, and one PR against `main`.
