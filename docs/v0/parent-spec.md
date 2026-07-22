## Problem Statement

Nodepad is currently a browser-based experiment whose complete state lives in `localStorage`. The root UI owns Thinking Workspace lifecycle, Note mutations, undo, persistence, AI orchestration, and migration behavior at once. Durable data is rewritten as whole JSON trees, provider secrets live in browser storage, URL enrichment depends on a running Next server route, domain types point inward to UI modules, and there are no automated tests.

The thinker needs a dependable macOS application that can be used every day to explore a topic spatially and associatively. It must preserve every Thinking Workspace locally across restarts, remain fully useful without AI, and optionally use local Ollama or explicitly consented Ollama Cloud assistance to organize Notes. V0 is a thinking tool, not a long-form writing environment.

## Solution

Build Nodepad V0 as a native macOS desktop application using Tauri 2, a React front end, Rust-owned native capabilities, and a local SQLite source of truth.

The application will organize atomic Markdown Notes inside Thinking Workspaces. The thinker can assign Note Types, Labels, Annotations, and Relationships manually and inspect the same material through tiling, kanban, and graph views. Search, undo, automatic durable saves, versioned archives, Markdown export, local backups, and restart recovery are core behavior.

AI Assistance is optional. Every Thinking Workspace has an explicit Assistance Policy: Manual, Local AI, or Cloud AI. Local AI uses the local Ollama host. Cloud AI uses Ollama Cloud only after Workspace-level consent. Both paths discover available models dynamically and share one normalized provider seam. AI may suggest Note Types, Labels, Annotations, Relationships, and Syntheses, but manual decisions always win and the application never becomes unusable when AI is absent or fails.

Architecturally, V0 deepens four modules: the Thinking Workspace module owns durable lifecycle and mutations; the Thinking Graph module owns Relationship invariants and projections; the Enrichment Workflow module owns optional AI organization; and the Document Workflow module owns native archives, backup, restore, and export. The highest test seam is the Thinking Workspace interface: tests submit user intents and verify returned durable state, including after closing and reopening the store.

## User Stories

1. As a thinker, I want to install Nodepad as a macOS application, so that I can use it without running a development server.
2. As a thinker, I want Nodepad to launch into a usable offline application, so that internet access is never required for manual thinking.
3. As a thinker, I want to create a Thinking Workspace for one topic, so that unrelated lines of thought stay separate.
4. As a thinker, I want to rename a Thinking Workspace, so that its evolving subject remains clear.
5. As a thinker, I want to delete a Thinking Workspace only after confirmation, so that a stray action cannot destroy a topic.
6. As a thinker, I want at least one valid Thinking Workspace to remain available, so that the application never opens into a broken state.
7. As a thinker, I want to create an atomic Markdown Note quickly from the keyboard, so that capture does not interrupt thought.
8. As a thinker, I want to edit a Note in place, so that I can refine an idea without creating duplicates.
9. As a thinker, I want to delete a Note, so that discarded thoughts stop cluttering the Workspace.
10. As a thinker, I want to pin an important Note, so that it remains prominent while I explore.
11. As a thinker, I want Note creation and editing to save automatically, so that I never need a manual save command.
12. As a thinker, I want committed Notes to survive quitting and reopening Nodepad, so that the desktop application is a durable thinking space.
13. As a thinker, I want a visible recovery message if the latest database cannot be opened, so that failure is actionable rather than silent.
14. As a thinker, I want to undo recent Note and Workspace mutations during my current session, so that exploration remains forgiving.
15. As a thinker, I want a fixed Note Type such as claim, question, idea, task, entity, quote, reference, definition, opinion, reflection, narrative, comparison, thesis, or general, so that structural roles remain consistent.
16. As a thinker, I want to change a Note Type manually, so that my judgment overrides automation.
17. As a thinker, I want to add multiple free-form Labels to a Note, so that I can organize subjects in my own language.
18. As a thinker, I want to rename or remove a Label, so that organization can evolve without rewriting Note text.
19. As a thinker, I want to write and edit an Annotation separately from the Note, so that contextual commentary does not alter the thought itself.
20. As a thinker, I want to create and remove Relationships between Notes manually, so that the Thinking Graph remains useful without AI.
21. As a thinker, I want Relationship changes to remain valid after editing, deleting, moving, or copying Notes, so that the Thinking Graph cannot accumulate dangling references.
22. As a thinker, I want to move a Note between Thinking Workspaces, so that I can correct where an idea belongs.
23. As a thinker, I want moved Notes to drop invalid cross-Workspace Relationships, so that each Thinking Graph remains self-contained.
24. As a thinker, I want to copy a Note into another Thinking Workspace, so that a thought can seed a different topic without sharing identity.
25. As a thinker, I want copied Notes to receive fresh identity and no inherited Relationships, so that later edits cannot corrupt the source Workspace.
26. As a thinker, I want a tiling view, so that I can read several Notes as a spatial field.
27. As a thinker, I want a kanban view grouped by Note Type, so that I can inspect the structural balance of my thinking.
28. As a thinker, I want a graph view derived from the Thinking Graph, so that hubs, isolated Notes, and clusters become visible.
29. As a thinker, I want all three views to show the same committed state, so that switching views never changes meaning.
30. As a thinker, I want selecting or hovering a Relationship to emphasize related Notes consistently across views, so that connections are easy to inspect.
31. As a thinker, I want to search Note text, Labels, and Annotations within a Workspace, so that I can recover an idea quickly.
32. As a thinker, I want search results to identify their Thinking Workspace and Note Type, so that matches retain context.
33. As a thinker, I want each new Thinking Workspace to start with Manual Assistance Policy, so that no thought content leaves my Mac by default.
34. As a thinker, I want Manual Assistance Policy to expose all organization controls, so that AI is never required.
35. As a thinker, I want Local AI to discover models from my local Ollama installation, so that I can choose from models actually available on my Mac.
36. As a thinker, I want Local AI to report clearly when Ollama is unavailable, so that a missing local process does not look like data loss.
37. As a thinker, I want Local AI to organize Notes without an API key, so that private offline assistance is straightforward.
38. As a thinker, I want Nodepad to recognize cloud-capable models exposed by a locally signed-in Ollama installation, so that Ollama can transparently offload supported models.
39. As a thinker, I want to connect directly to Ollama Cloud with an API key, so that I can use larger models without running Ollama locally.
40. As a thinker, I want Nodepad to fetch the current Ollama Cloud model list, so that retired and newly available models are not hard-coded.
41. As a thinker, I want to refresh, search, and select discovered Ollama models, so that model choice remains manageable.
42. As a thinker, I want my Ollama Cloud key stored in the macOS keychain, so that it is absent from SQLite, archives, logs, and UI source state.
43. As a thinker, I want an explicit disclosure before enabling Cloud AI for a Workspace, so that transmission of thought content is informed.
44. As a thinker, I want Cloud AI to receive only the active Note and a bounded set of relevant Notes, so that the entire database is never silently uploaded.
45. As a thinker, I want to return a Workspace to Manual Assistance Policy immediately, so that future AI requests stop at once.
46. As a thinker, I want AI to suggest a Note Type, Labels, an Annotation, and Relationships, so that organization requires less manual effort.
47. As a thinker, I want AI suggestions to remain editable and removable, so that the Thinking Workspace stays under my control.
48. As a thinker, I want manual changes protected from later AI enrichment, so that automation cannot overwrite my judgment.
49. As a thinker, I want failed or malformed AI responses to leave the original Note intact, so that provider problems cannot corrupt thought content.
50. As a thinker, I want to retry AI organization explicitly after a failure, so that recovery remains understandable.
51. As a thinker, I want Nodepad to propose a Synthesis only after enough related material exists, so that suggestions are meaningful rather than noisy.
52. As a thinker, I want to accept a Synthesis as a new Note, so that an emergent insight becomes part of the Workspace.
53. As a thinker, I want to dismiss a Synthesis, so that irrelevant suggestions leave no durable clutter.
54. As a thinker, I want repeated Syntheses to avoid near-duplicates, so that AI does not keep restating the same insight.
55. As a thinker, I want URL Notes to capture safe page metadata when reachable, so that references remain recognizable.
56. As a thinker, I want URL retrieval to reject local, private, metadata, and non-HTTP targets, so that a pasted URL cannot probe protected network resources.
57. As a thinker, I want to export one Thinking Workspace as readable Markdown, so that I can continue writing in another tool.
58. As a thinker, I want to export a versioned Nodepad archive, so that all Notes, Labels, Relationships, Annotations, and accepted state can move between installations.
59. As a thinker, I want archive import to validate its complete shape before changing SQLite, so that malformed data fails closed.
60. As a thinker, I want archive import to assign collision-safe identities, so that imported material cannot overwrite existing Notes.
61. As a thinker, I want provider secrets and transient AI request state excluded from archives, so that exports are safe and deterministic.
62. As a thinker, I want Nodepad to create rotating local database backups, so that accidental corruption has a recoverable path.
63. As a thinker, I want to restore from a chosen valid backup after confirmation, so that recovery does not silently replace current work.
64. As a thinker, I want migrations to create a backup before changing the database schema, so that upgrades remain recoverable.
65. As a keyboard-oriented thinker, I want command-palette and undo shortcuts to work consistently on macOS, so that common actions remain fast.
66. As a thinker, I want external links to open through the macOS-approved path, so that the desktop shell does not navigate away from my Workspace.
67. As a thinker, I want Nodepad to perform no telemetry or background cloud synchronization, so that local data ownership is clear.
68. As a maintainer, I want one decision-complete issue and a deterministic delivery workflow, so that a fresh agent can implement V0 without private conversation context.

## Implementation Decisions

- V0 targets macOS on Apple Silicon. Windows, Linux, Intel-specific packaging, mobile, and browser deployment are not release targets.
- Use Tauri 2 for the desktop shell, React and TypeScript for the user interface, and Rust for native capabilities. The application must launch without a Next development or production server.
- Reuse the proven visual language and interaction behavior where it still serves the accepted V0, but do not preserve the current state architecture or browser persistence for compatibility.
- The application is single-user, local-first, and account-free. It has no Nodepad backend, telemetry, cloud data sync, or automatic network backup.
- SQLite is the durable source of truth. Enable foreign keys and transactional migrations. Use WAL or an equivalent SQLite durability configuration appropriate to a desktop application.
- The initial schema represents Thinking Workspaces, Notes, Note Types, Labels, Note-to-Label membership, Relationships, Syntheses, Assistance Policy, non-secret provider preferences, archive metadata, and migration state.
- Store timestamps in an unambiguous machine format and render them in the user’s local timezone.
- Use stable collision-resistant identifiers generated by the application. Imported objects receive fresh identifiers while internal references are remapped transactionally.
- A Relationship can only connect two existing Notes in the same Thinking Workspace. Database constraints and Thinking Graph rules must prevent dangling and cross-Workspace Relationships.
- Preserve Relationship provenance as manual or AI-suggested. Manual Relationships are never silently replaced by later AI output.
- The fixed Note Type taxonomy is: claim, question, idea, task, entity, quote, reference, definition, opinion, reflection, narrative, comparison, thesis, and general.
- A task is a Note Type in V0. Specialized task-management behavior, shared task containers, due dates, and subtask workflows are not part of this spec.
- Labels are free-form and scoped to a Thinking Workspace. Label identity is case-normalized while preserving a display name. Duplicate membership is prevented.
- Annotation is separate from Note text. Track whether the latest Annotation or Note Type choice was manually authored so AI cannot overwrite it without an explicit user request.
- The Thinking Workspace module is the highest application seam. Its interface accepts explicit user intents and returns a committed outcome or typed failure plus the resulting durable snapshot needed by callers.
- Thinking Workspace intents cover Workspace lifecycle, Note lifecycle, pinning, Note Type and Annotation edits, Label membership, Relationship edits, move/copy, Synthesis acceptance/dismissal, import, restore, and session undo.
- A successful intent means its SQLite transaction committed. UI state must not claim success before durable completion.
- The UI observes committed snapshots or narrowly scoped committed changes from the Thinking Workspace module. Views do not write SQLite or create native capabilities directly.
- Keep recent reversible mutations in a bounded session undo log. Undo creates a new compensating transaction; undo history need not survive application restart.
- The SQLite storage adapter and an in-memory adapter satisfy the same storage seam. The in-memory adapter exists for conformance testing, not as a production persistence mode.
- A one-time browser `localStorage` migration is deliberately absent. V0 starts from a fresh database.
- The Thinking Graph module owns Relationship validation, deletion cleanup, move/copy semantics, degree calculation, related-Note lookup, and the projections consumed by tiling, kanban, and graph views.
- Every view is a projection of the same committed Thinking Workspace state. View-local selection, hover, zoom, and layout positions are transient unless explicitly promoted into durable domain behavior later.
- Tiling remains an automatic spatial reading surface rather than a freeform coordinate canvas. Kanban groups by Note Type. Graph layout is derived from the Thinking Graph.
- Search uses SQLite full-text search across Note text and Annotation with Label joins. Search is scoped by default to the active Thinking Workspace and may expose an explicit all-Workspace option.
- Every Thinking Workspace stores one Assistance Policy: Manual, Local AI, or Cloud AI. New Workspaces default to Manual.
- Changing Assistance Policy to Manual cancels or invalidates pending enrichment results and prevents new AI requests. A stale response must never apply after the Note, model, Workspace, or policy changed.
- V0 supports two AI endpoints only: local Ollama and direct Ollama Cloud. Existing OpenRouter, OpenAI, and Z.ai provider implementations are not carried into the V0 architecture.
- Local Ollama defaults to `http://localhost:11434`; direct Ollama Cloud defaults to `https://ollama.com`. A user-editable Ollama-compatible host may be retained as an advanced preference if it does not weaken Tauri capability restrictions.
- Use Ollama’s native model-listing and chat behavior. Model discovery uses the tags endpoint; enrichment and Synthesis use the chat endpoint with structured JSON output where the selected model supports it.
- Support local models, cloud-capable models exposed through a locally signed-in Ollama installation, and models available through direct Ollama Cloud.
- Model lists are fetched dynamically, refreshable, searchable, and never treated as a permanent hard-coded catalog. Persist the selected model as a preference and handle its later disappearance explicitly.
- Direct Ollama Cloud uses a bearer key stored only in the macOS keychain. Never persist or log the key in SQLite, archives, crash output, React state snapshots, or command arguments visible to other processes.
- Cloud AI requires explicit per-Workspace disclosure and consent. The UI must make the active Assistance Policy and selected model visible.
- Enrichment sends the active Note plus no more than ten relevant Notes from the same Thinking Workspace. Selection is recency-biased and diversity-aware. Apply explicit text-size limits before transmission.
- No AI path may read Notes from other Thinking Workspaces unless a later user-approved feature changes the domain model.
- AI enrichment may suggest Note Type, Labels, Annotation, Relationships, and unrelated status. It must return normalized structured data that the Enrichment Workflow validates before committing.
- AI-suggested Relationships reference stable Note identities after context-index normalization. Invalid or unknown references are rejected rather than guessed.
- AI failures are typed and visible: unavailable host, authentication failure, missing model, retired model, timeout, rate limit, malformed output, and cancelled/stale request. The original Note remains committed and editable.
- Synthesis requires at least five enriched Notes, at least two represented Note Types or Labels, five new qualifying Notes since the prior attempt, and a cooldown. Keep at most five pending Syntheses per Workspace and use recent Synthesis text to prevent near-duplicates.
- The thinker must explicitly accept a Synthesis before it becomes a Note. Dismissal removes the pending Synthesis without altering source Notes.
- Native URL retrieval replaces the Next server route. Allow only HTTP and HTTPS, enforce timeouts and response-size limits, validate redirects, reject loopback/private/link-local/metadata/reserved destinations after DNS resolution, and accept only safe textual metadata extraction.
- Provider requests, URL retrieval, keychain access, file dialogs, external-link opening, SQLite access, and backup operations are explicit Tauri capabilities. Do not add a shallow generic platform pass-through.
- The Document Workflow module owns versioned archive validation, export, import, Markdown rendering, manual backup, rotating automatic backup, and restore behavior.
- Define a new V0 archive format with an explicit version and complete validation. There is no obligation to import the legacy browser archive format.
- Archives contain durable domain state only. Exclude secrets, transient selection/layout state, undo history, in-flight AI state, and local database metadata.
- Import validates the complete archive before opening a transaction. It either commits all remapped data or changes nothing.
- Markdown export is human-readable and organized by Note Type and Labels while retaining Annotations, Relationships, sources, and timestamps sufficiently for writing elsewhere.
- Create a safe database backup before every schema migration. Create at most one automatic backup per day when data changed and retain the latest seven valid automatic backups.
- Restore requires explicit confirmation, validates the chosen backup, preserves the current database as a pre-restore backup, and restarts or reloads durable state cleanly.
- Provider secrets rely on the macOS keychain and normal full-disk protection such as FileVault. Separate app-password locking and database-level encryption are deferred.
- Remove static build-error suppression. TypeScript errors, Rust compilation errors, lint failures, and test failures must block the V0 gate.
- Do not add production telemetry, analytics, remote logging, or background update checks in this issue.
- Packaging must produce an installable macOS artifact suitable for local use. Code signing/notarization may remain documented manual steps if credentials are unavailable to the implementation agent, but the unsigned local build must be verified.

## Testing Decisions

- Good tests exercise externally observable behavior through the highest practical seam. They do not assert React implementation details, private helper calls, SQL statement text, Tauri command names, prompt prose, or internal module arrangement.
- The primary suite exercises the Thinking Workspace interface by submitting user intents and inspecting committed outcomes and snapshots.
- Every durability scenario that matters must close the storage adapter, reopen it, and assert recovered state. An in-memory-only assertion is insufficient evidence for persistence behavior.
- Run the same storage conformance suite against SQLite and the in-memory adapter for shared semantics: create, update, delete, move, copy, labels, Relationships, transactions, failures, and identifier constraints.
- Test transaction rollback by injecting storage failures and verifying that neither partial state nor success UI outcomes escape.
- Test schema initialization from an empty app-data directory, sequential migrations, backup-before-migration, failed migration recovery, and reopening the latest supported schema.
- Test Thinking Graph behavior through domain outcomes: dangling-Relationship cleanup, cross-Workspace rejection, move/copy remapping, deduplication, degree calculation, and consistent projections.
- Test manual operation end to end with no Ollama process, no network, and no keychain credential. All core creation, organization, search, views, persistence, archive, and export behavior must remain available.
- Provider adapter contract tests cover local Ollama and direct Ollama Cloud using controlled HTTP fixtures rather than live billable models.
- Provider tests cover model listing, refresh, selection, empty lists, missing selected model, authentication failure, retired model, timeout, rate limit, malformed JSON, structured output, cancellation, and stale responses.
- Keychain tests prove secrets round-trip through the secret seam and never appear in serialized preferences, database rows, archive output, logs, or error messages.
- Cloud-consent tests prove a Manual or Local AI Workspace cannot invoke direct Ollama Cloud and that changing policy invalidates pending cloud results.
- Context-selection tests prove cloud requests contain only the active Note plus at most ten Notes from the same Thinking Workspace and respect text-size limits.
- Manual-override tests prove later AI results cannot overwrite manually assigned Note Type, Labels, Annotation, or Relationships.
- Synthesis tests cover eligibility thresholds, cooldown, duplicate prevention, pending cap, accept, dismiss, and failure without Note corruption.
- Native URL retrieval tests cover allowed public URLs, redirect chains, timeout, oversized responses, non-text content, malformed HTML, DNS rebinding defenses where practical, loopback, private IPv4/IPv6, link-local, metadata hosts, and reserved ranges.
- Archive tests cover round-trip fidelity, Unicode, large Notes, invalid versions, missing fields, duplicate identifiers, broken Relationships, collision remapping, cancellation, and transaction rollback.
- Backup and restore tests cover retention, unchanged-day behavior, invalid backup rejection, pre-restore safety backup, and restart recovery.
- UI integration tests cover the smallest critical paths: first launch, create Workspace, create/edit Note, manual organization, switch views, search, configure each Assistance Policy, select a model, export, import, quit, and reopen.
- Keyboard tests cover capture, command palette, undo, escape/dismissal, and focus behavior on macOS.
- Accessibility checks cover keyboard reachability, visible focus, semantic labels, modal focus trapping, reduced-motion behavior, and sufficient status communication without color alone.
- Add a packaging smoke test that builds the Tauri app and launches it against a temporary app-data directory.
- The repository currently has no tests. There is no existing test prior art to preserve; establish the Thinking Workspace conformance suite as the canonical pattern for later work.
- Final evidence must include focused test results, TypeScript checking, lint, Rust formatting and linting, Rust tests, the production front-end build, the Tauri build, and the scoped Fallow report.
- A passing web-only build is not completion. V0 is complete only when the macOS desktop artifact launches and manual offline persistence survives restart.

## Out of Scope

- Long-form drafting, document composition, rich text, publishing, and manuscript management.
- Cloud synchronization, Nodepad accounts, shared Workspaces, collaboration, comments, presence, or conflict resolution.
- Windows, Linux, Intel-specific release packaging, mobile, and continued browser deployment.
- Importing browser `localStorage` or guaranteeing compatibility with legacy `.nodepad` files.
- OpenRouter, OpenAI, Z.ai, Anthropic, or other non-Ollama provider adapters.
- Pulling, deleting, creating, copying, publishing, or otherwise managing models in an Ollama installation.
- Web search or research-agent behavior beyond safe metadata retrieval for an explicit URL Note.
- A freeform coordinate canvas, whiteboard drawing, image attachments, audio, video, and arbitrary file embedding.
- Specialized task management, subtasks, due dates, reminders, calendars, notifications, or project planning.
- App-password locking, database-level encryption, biometric unlock, secret recovery, and key rotation beyond macOS keychain behavior.
- Telemetry, analytics, remote crash reporting, advertising, or automated remote logging.
- Automatic application updates, App Store distribution, and release credential acquisition.
- Persistent undo across restarts, full Note version history, or event sourcing.
- Cross-Workspace Relationships or AI context spanning multiple Thinking Workspaces.
- Performance work aimed at very large collaborative or server-scale datasets.
- Cleaning unrelated pre-existing dead code or broadly redesigning untouched legacy behavior.

## Further Notes

- Canonical domain language is Thinking Workspace, Note, Note Type, Label, Annotation, Relationship, Thinking Graph, Synthesis, AI Assistance, and Assistance Policy. Avoid the legacy synonyms recorded in the repository glossary.
- The accepted architecture review recommends the Durable Thinking Workspace first, followed by Thinking Graph invariants, Enrichment Workflow, and Desktop Document Workflow.
- Ollama’s local and cloud hosts expose the same native interface shape. Local access requires no authentication; direct cloud access uses a bearer key. Models must be discovered dynamically because cloud models may be added or retired.
- Product data remains local even when Cloud AI is enabled. Cloud AI consent covers bounded inference requests only; it does not enable Nodepad sync, backup, telemetry, or storage.
- The current UI is behavioral reference material, not an architectural constraint. Preserve valuable interactions while replacing shallow state and runtime assumptions.
- This issue is intentionally one V0 delivery slice because the requested operating model is one audited issue, one fresh implementation session, one review pass, one scoped Fallow pass, and one PR. The simplicity audit may recommend internal milestones but must not fragment accepted outcomes without maintainer approval.

### Mandatory agent delivery workflow

1. Read this complete issue, the repository instructions, the domain glossary, and relevant ADRs.
2. Run the `prd-simplicity-audit` skill before implementation. Review every recommendation and simplify the plan where it preserves the accepted product outcomes. Record material audit decisions in an issue comment.
3. Start a fresh agent session, load the `implement` skill, and build only this issue.
4. After implementation and focused tests, load the `code-review` skill. Review the complete issue diff and address all actionable findings.
5. Load the `fallow` skill. Scope analysis strictly to code introduced or modified for this issue; pre-existing dead code is explicitly excluded.
6. Run the complete repository gate and build the macOS Tauri artifact.
7. Push a scoped branch and open a pull request against `main`. Link this issue and include audit, test, review, Fallow, and build evidence.
8. Do not merge the pull request without review.
