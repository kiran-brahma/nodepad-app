# Nodepad V0 planning backup

This folder preserves the complete local planning record for Nodepad V0 as published on 22 July 2026.

## Primary artifacts

- [Architecture review](./architecture-review.html) — visual deep-module review that selected the Durable Thinking Workspace as the first seam.
- [Parent specification](./parent-spec.md) — local backup of [GitHub Issue #1](https://github.com/kiran-brahma/nodepad-app/issues/1).
- [Approved AI prompt contracts](./ai-prompt-contracts.md) — reviewed Prompt A for Note Organization and Prompt B for Synthesis.
- [Child-issue plan](./child-issue-plan.md) — approved 18-slice decomposition and dependency graph.
- [Child issue bodies](./issues/) — final bodies published to GitHub with actual parent and blocker issue numbers.

## Child issue index

| Slice | GitHub | Local backup |
| --- | --- | --- |
| V0-01 | [#2 Bootstrap Tauri and persist the first Note](https://github.com/kiran-brahma/nodepad-app/issues/2) | [01-foundation.md](./issues/01-foundation.md) |
| V0-02 | [#3 Thinking Workspace lifecycle and recovery](https://github.com/kiran-brahma/nodepad-app/issues/3) | [02-workspace-lifecycle.md](./issues/02-workspace-lifecycle.md) |
| V0-03 | [#4 Manual Note controls, Annotation, pinning, and undo](https://github.com/kiran-brahma/nodepad-app/issues/4) | [03-note-controls.md](./issues/03-note-controls.md) |
| V0-04 | [#5 Workspace Labels and active-Workspace search](https://github.com/kiran-brahma/nodepad-app/issues/5) | [04-labels-search.md](./issues/04-labels-search.md) |
| V0-05 | [#6 Symmetric Relationships and Thinking Graph invariants](https://github.com/kiran-brahma/nodepad-app/issues/6) | [05-relationships.md](./issues/05-relationships.md) |
| V0-06 | [#7 Move and copy Notes safely](https://github.com/kiran-brahma/nodepad-app/issues/7) | [06-move-copy.md](./issues/06-move-copy.md) |
| V0-07 | [#8 Durable tiling and kanban projections](https://github.com/kiran-brahma/nodepad-app/issues/8) | [07-tiling-kanban.md](./issues/07-tiling-kanban.md) |
| V0-08 | [#9 Graph view and Relationship focus](https://github.com/kiran-brahma/nodepad-app/issues/9) | [08-graph-view.md](./issues/08-graph-view.md) |
| V0-09 | [#10 Assistance Policy and local Ollama discovery](https://github.com/kiran-brahma/nodepad-app/issues/10) | [09-local-ollama.md](./issues/09-local-ollama.md) |
| V0-10 | [#11 Ollama Cloud consent, keychain, and discovery](https://github.com/kiran-brahma/nodepad-app/issues/11) | [10-ollama-cloud.md](./issues/10-ollama-cloud.md) |
| V0-11 | [#12 Automatic Note Organization with Prompt A](https://github.com/kiran-brahma/nodepad-app/issues/12) | [11-note-organization.md](./issues/11-note-organization.md) |
| V0-12 | [#13 Provisional Synthesis with Prompt B](https://github.com/kiran-brahma/nodepad-app/issues/13) | [12-synthesis.md](./issues/12-synthesis.md) |
| V0-13 | [#14 Safe native URL metadata enrichment](https://github.com/kiran-brahma/nodepad-app/issues/14) | [13-url-metadata.md](./issues/13-url-metadata.md) |
| V0-14 | [#15 Native Markdown export](https://github.com/kiran-brahma/nodepad-app/issues/15) | [14-markdown-export.md](./issues/14-markdown-export.md) |
| V0-15 | [#16 Versioned Nodepad archive workflow](https://github.com/kiran-brahma/nodepad-app/issues/16) | [15-archive.md](./issues/15-archive.md) |
| V0-16 | [#17 Rotating backup, restore, and migration safety](https://github.com/kiran-brahma/nodepad-app/issues/17) | [16-backup-restore.md](./issues/16-backup-restore.md) |
| V0-17 | [#18 macOS keyboard, accessibility, and external links](https://github.com/kiran-brahma/nodepad-app/issues/18) | [17-macos-interactions.md](./issues/17-macos-interactions.md) |
| V0-18 | [#19 Privacy gates and macOS artifact](https://github.com/kiran-brahma/nodepad-app/issues/19) | [18-release.md](./issues/18-release.md) |

## Repository operating documents

The V0 artifacts depend on the repository-level [agent instructions](../../AGENTS.md), [domain glossary](../../CONTEXT.md), and [agent configuration](../agents/).

GitHub remains the active tracker. These files are a version-controlled backup and decision record; GitHub issue state, native dependencies, discussion, and implementation progress remain authoritative for execution.
