## Parent

Part of #1.

## What to build

Harden the integrated V0, prove its privacy and offline guarantees, run the complete release gate, and produce a launchable Apple Silicon macOS artifact. This slice fixes only integration/release defects discovered in the V0 paths; it does not add product scope.

## Decisions

- Build Apple Silicon macOS only.
- No telemetry, analytics, cloud sync, remote logging, background update checks, or non-Ollama provider traffic.
- Manual mode must operate with network disabled and no keychain credential.
- Direct network allowlist is local Ollama, Ollama Cloud when consented, explicit safe URL metadata targets, and user-opened HTTP(S) links.
- Secrets never appear in SQLite, archives, backups, logs, errors, UI snapshots, process arguments, or generated artifacts.
- Produce an unsigned local `.app`/DMG as supported by Tauri when signing credentials are unavailable. Document signing/notarization commands and required secrets without fabricating them.
- Do not weaken gates to ship: TypeScript, lint, Rust format/lint/tests, front-end production build, Tauri build, focused integration tests, and packaging smoke must pass.

## Acceptance criteria

- [ ] Fresh install launches against empty app data and creates the valid initial Workspace.
- [ ] Manual offline smoke covers create/edit/type/Label/Annotation/Relationship, all views, search, export, quit, reopen, and exact durable recovery.
- [ ] Local Ollama and consented Ollama Cloud fixture smoke paths work; Manual makes no provider request.
- [ ] Synthesis, URL metadata, archive round-trip, backup, and restore integration paths pass.
- [ ] Sentinel-secret audit finds no key material in every prohibited location.
- [ ] Network audit finds no telemetry, sync, updater, hidden provider, or unconsented cloud request.
- [ ] TypeScript, lint, Rust formatting/lint/tests, UI/integration tests, production build, and Tauri build all pass without ignored errors.
- [ ] The Apple Silicon artifact installs/opens locally and the packaged app passes restart persistence.
- [ ] Signing/notarization status is explicit; missing credentials are the only permitted reason those external steps remain manual.
- [ ] Release notes list V0 scope and exclusions without claiming Windows/Linux/browser support.

## Testing decisions

- Run all predecessor focused suites plus one clean app-data end-to-end smoke.
- Use offline/network interception to prove allowed/forbidden traffic.
- Scan built assets, logs, databases, archives, and backups for sentinel secrets and development URLs.
- Run `code-review` on the integrated issue diff and `fallow` only on code added/changed in this release slice; predecessor review evidence remains linked.

## Blocked by

- #7
- #9
- #11
- #12
- #13
- #14
- #15
- #16
- #17
- #18

## Scope fence

No new features, broad dependency upgrades, unrelated dead-code cleanup, visual redesign, Windows/Linux work, App Store publication, cloud sync, or provider expansion.

## Delivery workflow

Run `prd-simplicity-audit`, use a fresh `implement` session, then `code-review`, scoped `fallow`, the complete release gate, and one PR against `main`. Do not merge without review.
