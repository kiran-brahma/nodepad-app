# Nodepad

**A personal thinking tool for developing a topic spatially and associatively — before writing.**

Nodepad is a native macOS application. Tauri 2 hosts a React interface; SQLite on your Mac is the durable source of truth. No server, no account, no telemetry, no browser storage. AI assistance is optional and off by default.

Most AI tools put a chat box at the centre. Nodepad puts your thinking there and keeps assistance in the background: you capture atomic thoughts, and AI — when you enable it — classifies, annotates, proposes links, and occasionally offers a synthesis. Every assisted result stays editable, and the app is fully usable with AI switched off.

---

## Quick start

Requirements: current Node.js, Rust, and the macOS Tauri build prerequisites.

```bash
npm install
```

```bash
npm run dev
```

`npm run dev` launches the Tauri application. On first launch Nodepad creates its SQLite database in the macOS application-data directory, applies numbered migrations, and creates one default Thinking Workspace.

---

## Concepts

Nodepad has a deliberate vocabulary. The full glossary is [CONTEXT.md](CONTEXT.md); the short version:

| Term | Meaning |
| --- | --- |
| **Thinking Workspace** | A self-contained map of one topic. |
| **Note** | An atomic thought, in Markdown, with whatever classification and commentary you attach. |
| **Note Type** | A fixed structural role: claim, question, task, idea, entity, quote, reference, definition, opinion, reflection, narrative, comparison, thesis, general. |
| **Label** | A free-form name you assign to organize Notes by subject. |
| **Annotation** | Commentary attached to a Note that adds context without changing the thought. |
| **Relationship** | A meaningful association between two Notes in the same Workspace. |
| **Thinking Graph** | The set of Relationships in a Workspace, independent of how it is drawn. |
| **Synthesis** | A provisional insight across several Notes that you may accept as a Note. |
| **Assistance Policy** | Per-Workspace: Manual, Local AI, or Cloud AI. |

---

## What it does

**Capture.** A capture bar is pinned to the foot of the window. Enter commits, Shift+Enter adds a line. Notes save automatically and survive a restart.

**Organize.** Set a Note Type, attach Labels, write an Annotation, pin what matters, relate Notes to each other, and move or copy a Note into another Workspace. Cross-Workspace Relationships are dropped on a move, so each Thinking Graph stays self-contained.

**Arrange.** The same committed Notes, three ways — **Canvas** (spatial, with durable positions and drawn Relationships), **Kanban** (grouped by Note Type, drag to reclassify), **Graph** (force-directed over the Thinking Graph). Switching view never changes meaning. Focusing a Note lights it and everything related to it, and dims the rest, identically in every view.

**Recover.** Session undo, versioned `.nodepad` archives, Markdown export, rolling local backups, and a recovery screen if the database cannot be opened.

**Assist (optional).** Each Workspace starts Manual — nothing leaves your Mac. Switch it to Local AI (your local Ollama install) or Cloud AI (after an explicit per-Workspace consent disclosure). Models are discovered dynamically, never hard-coded. Cloud keys are held in the macOS keychain, never in SQLite, archives, or logs.

---

## Keyboard

The app is drivable without the mouse.

| | |
| --- | --- |
| `Enter` | Commit the Note in the capture bar |
| `Shift+Enter` | New line inside a Note |
| `⌘K` | Command palette |
| `⌘.` | Command menu on the focused Note card |
| `⌘Z` / `Ctrl+Z` | Undo the last change |
| `Escape` | Dismiss the topmost surface — palette, modal, inline draft, then focus |

`⌘K` covers the whole application: switch view, focus the capture bar, open Workspace settings, switch Workspace by name, rename/delete/export/import, set the Assistance Policy, and act on the focused Note (edit, relate, pin, set Note Type, delete). Note actions are listed but disabled while no Note is focused.

---

## Data and privacy

- SQLite in the macOS application-data directory is the only durable store. Numbered migrations under `src-tauri/migrations/` own the schema.
- Cloud API keys live in the macOS keychain, separate from the database and from any export.
- Manual Workspaces make no network calls at all. Local AI talks only to your local Ollama host. Cloud AI requires recorded consent before a single request; revoking consent returns the Workspace to Manual.
- Export a Workspace as Markdown or as a versioned `.nodepad` archive; import an archive back.

---

## Development

```bash
npm run check
```

Runs the full gate: TypeScript, ESLint, Vitest, `cargo fmt`, Clippy, Rust tests, and a Tauri compile smoke check. `npm run build` runs the gate and then produces the macOS artifact.

Useful subsets:

```bash
npm run test
```

```bash
npm run typecheck
```

### Layout

```text
src/                  React interface, one module per surface
  workspace-client.ts   the single frontend seam over the Rust commands
  *.test.ts(x)          Vitest, driven through that seam
src-tauri/
  src/                  Rust: workspace, thinking graph, enrichment, synthesis,
                        archive, backup, export, secrets, cloud
  migrations/           numbered SQL migrations
docs/
  agents/               repository process for engineering agents
  audits/               per-issue PRD simplicity audits
  v0/                   the V0 specification and issue plan
CONTEXT.md            the domain glossary — the vocabulary all code uses
AGENTS.md             the delivery workflow
CLAUDE.md             working notes for Claude Code
```

### Architecture in one paragraph

Durable rules live in Rust and are enforced against SQLite; the frontend owns no business rule it could disagree about. `src/workspace-client.ts` is the one seam between them, and it is also the one test seam — tests submit user intents through the DOM and assert on the state that comes back. Within the frontend, each concern has exactly one home: what may be done to a Note is `note-intents.ts`, what the ⌘K palette offers is `palette-actions.ts`, which Note is focused is `note-focus.ts`. Surfaces call into those; they never restate them.

---

## Contributing

Work is tracked in GitHub Issues, not in pull requests. See [AGENTS.md](AGENTS.md) for the delivery workflow and [docs/agents/](docs/agents/) for the issue-tracker and triage conventions. Pull requests are welcome; each should trace to an issue, keep to one observable path, ship its own tests, and pass `npm run check`.

---

## Credits

Nodepad began as a browser-based design experiment by [Saleh Kayyali](http://mskayyali.com). This repository is the native macOS rebuild.

[Watch the original intro →](https://www.youtube.com/watch?v=jZu4sgZOOO4)

## License

[MIT](LICENSE)
