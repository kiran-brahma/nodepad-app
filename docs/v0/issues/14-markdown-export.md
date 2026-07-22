## Parent

Part of #1.

## What to build

Add native Markdown export for one Thinking Workspace. The thinker chooses a destination with a macOS save dialog. The Document Workflow renders a deterministic, readable document suitable for continuing long-form writing elsewhere.

## Decisions

- Export one active Workspace per file.
- Filename defaults to a sanitized Workspace name plus `.md`; preserve Unicode where safe.
- Document order: title/metadata, Note-Type sections in fixed taxonomy order, then Notes pinned-first and creation-ordered.
- Each Note includes stable export anchor, Note text, optional Annotation, Labels, Note Type, local timestamp, sources, and a readable list of related Note anchors/titles.
- Do not expose database IDs directly as prose; internal anchors may use safe generated tokens.
- Escape Markdown tables/links/content only where exporter-generated syntax requires it; preserve authored Markdown body.
- Native save cancellation is a successful no-op.
- Write atomically through a temporary sibling and rename where supported.

## Acceptance criteria

- [ ] The thinker can choose a location and export the active Workspace.
- [ ] Output is deterministic for unchanged durable state.
- [ ] Every Note, Label, Annotation, Note Type, source, timestamp, and Relationship is represented readably.
- [ ] Authored Markdown remains meaningful and exporter metadata cannot corrupt structure.
- [ ] Unicode Workspace names and Notes export correctly.
- [ ] Cancellation and write failure leave no misleading success state or partial destination.
- [ ] Export contains no secret, AI request state, undo history, database path, or transient view state.
- [ ] The file opens as plain Markdown outside Nodepad.

## Testing decisions

- Golden tests cover every Note Type, Labels, Annotation, symmetric Relationships, source URLs, Unicode, Markdown edge cases, empty Workspace, and deterministic order.
- File adapter tests cover cancellation, atomic write, collision/overwrite confirmation, and permission failure.
- UI integration covers native dialog through an adapter without hard-coding macOS dialog internals.

## Blocked by

- #5
- #6

## Scope fence

Do not add long-form editing, PDF/HTML export, multi-Workspace export, publishing, templates, or legacy formatting compatibility.

## Delivery workflow

Run `prd-simplicity-audit`, use a fresh `implement` session, then `code-review`, scoped `fallow`, focused tests and repository gates, and one PR against `main`.
