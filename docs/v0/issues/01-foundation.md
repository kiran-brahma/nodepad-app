## Parent

Part of #1.

## What to build

Deliver the first complete desktop tracer bullet: an Apple Silicon macOS Tauri 2 application that launches without a Next server, creates one Thinking Workspace and one atomic Markdown Note, commits both through the Thinking Workspace interface into SQLite, and recovers them after the application and database are closed and reopened.

This issue establishes the architecture every later slice extends. React/TypeScript owns rendering and user interaction. Rust owns native capabilities and the app-data location. SQLite is the durable source of truth. Callers submit explicit intents to the Thinking Workspace module and receive a committed outcome or typed failure plus the durable state needed to render. UI state must not report success before the SQLite transaction commits.

Land the already-approved repository operating contract and domain glossary as part of this baseline. Establish blocking TypeScript, Rust, lint, and test commands. Remove intentional build-error suppression. Do not migrate the whole legacy UI; reuse only the minimum visual language needed for a recognizable capture path.

## Decisions

- Target Tauri 2 on Apple Silicon macOS.
- Use a static React/TypeScript front end; no Next server or production web deployment.
- Use SQLite with foreign keys, WAL mode, transactional migrations, and collision-resistant application-generated IDs.
- Initial durable entities are Thinking Workspace and Note. Add future entities only through later slice migrations.
- A Note stores identity, Workspace identity, Markdown text, fixed Note Type, timestamps, and pinned state. Default Note Type is `general`.
- Create a default Thinking Workspace on an empty database, but expose an explicit create-Workspace action in the minimal UI.
- A successful create intent means the transaction committed.
- The storage seam has SQLite and in-memory adapters. The in-memory adapter exists only for conformance tests.
- No browser `localStorage` migration or legacy archive compatibility.
- No AI, Labels, Relationships, multiple views, import/export, or backup behavior in this slice.

## Acceptance criteria

- [ ] A documented development command launches the Tauri app on Apple Silicon macOS without a Next server.
- [ ] First launch initializes an empty SQLite database in the macOS application-data location through a numbered migration.
- [ ] Foreign keys are enabled and the database uses WAL mode.
- [ ] An empty database yields one valid default Thinking Workspace.
- [ ] The thinker can create another Thinking Workspace through the minimal UI.
- [ ] The thinker can create an atomic Markdown Note in the active Workspace.
- [ ] Workspace and Note creation go through one Thinking Workspace interface rather than direct UI database calls.
- [ ] The UI renders committed data and shows a typed, recoverable failure when a transaction fails.
- [ ] Closing the adapter/application and reopening it recovers the exact committed Workspace and Note.
- [ ] The same create/read semantics pass against SQLite and the in-memory adapter.
- [ ] Repository instructions, domain glossary, and agent configuration are present and consistent with the approved workflow.
- [ ] TypeScript errors, Rust compilation errors, lint failures, and tests block the build.
- [ ] No production telemetry, account, cloud sync, provider request, or browser persistence is introduced.

## Testing decisions

- Add the canonical Thinking Workspace conformance harness. Submit create intents and inspect committed outcomes; do not test private functions or SQL strings.
- Run shared create/read tests against both adapters.
- SQLite durability tests must close and reopen a temporary database before asserting recovery.
- Inject a transaction failure and prove no partial Workspace or Note escapes and no success outcome is returned.
- Add a Tauri launch smoke test or the narrowest automatable equivalent supported by the toolchain.

## Blocked by

None - can start immediately.

## Scope fence

Build the smallest durable vertical path. Do not port the complete legacy page, views, AI, export, URL retrieval, settings, or styling system. Do not clean unrelated legacy code.

## Delivery workflow

Run `prd-simplicity-audit`, record material audit decisions, then use a fresh `implement` session. After focused tests, run `code-review`, fix actionable findings, run `fallow` only on introduced or modified code, run the complete available gate, and open one scoped PR against `main`. Do not merge without review.
