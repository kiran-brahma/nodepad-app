# Domain Docs

Nodepad is a single-context repository. Engineering agents must use its domain language consistently.

## Before exploring

- Read `CONTEXT.md` at the repository root.
- Read ADRs under `docs/adr/` that touch the work area.
- If a referenced document does not exist, proceed silently. Domain documents are created lazily when terms or durable decisions are resolved.

## Layout

```text
/
├── CONTEXT.md
├── docs/
│   ├── adr/
│   └── agents/
└── app and runtime modules
```

The presence of `CONTEXT-MAP.md` would indicate a future move to multiple contexts. Until then, do not invent context-specific glossaries.

## Use the glossary vocabulary

Use terms exactly as defined in `CONTEXT.md` in issue titles, specifications, implementation names, tests, and reviews. Do not substitute synonyms listed under `_Avoid_`.

If a required domain concept is absent, use the `domain-modeling` skill to resolve and record it rather than silently inventing competing vocabulary.

## ADR conflicts

If proposed work contradicts an existing ADR, identify the conflict explicitly. Do not silently override or re-litigate a durable decision.
