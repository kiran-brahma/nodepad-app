# PRD SIMPLICITY AUDIT

Feature: V0-18 — Harden privacy gates and build the macOS artifact
Issue: kiran-brahma/nodepad-app#19
Date: 2026-07-24
Gate: **PROCEED WITH MODIFICATIONS**

---

## MODULE MAP

This slice adds no product modules. It adds audit modules, repairs gate
failures, and produces a build artifact.

### Existing modules (touched)

- `src-tauri/src/workspace.rs` — durable state. Owns the existing
  `no_durable_state_carries_a_sentinel_bearer_key` test, which is the seed of
  the release's sentinel audit. Also carries one dead private function and one
  redundant `mut` that fail the Rust lint gate.
- `src-tauri/src/secrets.rs` — the one keychain seam. The audit's claim that
  "the keychain is the only place a bearer key lives" is a claim about this
  module's boundary, so the audit belongs next to it, not scattered.
- `src-tauri/src/cloud.rs` / `ollama.rs` / `url_metadata.rs` /
  `external_link.rs` — the four and only four places an outbound request or an
  outbound URL is decided. The network audit's whole job is to prove this list
  is exhaustive and stays exhaustive.
- `src-tauri/src/backup.rs`, `archive.rs`, `markdown_export.rs` — the three
  artifact writers a secret could leak into.
- `src-tauri/tauri.conf.json` — bundle targets and CSP. Ships `targets: ["app"]`
  with an empty `icon` array today.
- `package.json` — the `check` script. Today it is *not* the release gate the
  issue describes: it omits `cargo fmt` and `cargo clippy` entirely.

### New modules

- `src-tauri/src/release_audit.rs` — owns both release audits as ordinary Rust
  tests over the real source tree and real artifacts. One module carries
  "what may leave Nodepad" (host allowlist) and "what a secret may never touch"
  (sentinel locations), so a future change to either rule edits one file.
- `docs/v0/release-notes.md` — V0 scope, exclusions, and the explicit
  signing/notarization status.

---

## INTERROGATION FINDINGS

### The release gate does not exist yet

**COMPLECTED → must fix first.** The issue's central non-negotiable is "do not
weaken gates to ship: TypeScript, lint, Rust format/lint/tests, front-end
production build, Tauri build, focused integration tests, and packaging smoke."
The repository's `npm run check` runs typecheck, eslint, vitest, `cargo test`,
and a `cargo check` smoke. It never runs `cargo fmt` or `cargo clippy`.

Verified against the tree: `cargo fmt --check` reports diffs in
`src-tauri/src/backup/tests.rs`, and `cargo clippy --all-targets -D warnings`
reports **11 errors**. The gate the issue treats as the ship condition has been
failing for the whole of V0 and nothing surfaced it, because nothing ran it.

This is the audit's most important finding: the acceptance criterion "all pass
without ignored errors" cannot be evaluated until the gate is a single runnable
command. Fix the failures *and* wire the gate, or the criterion is unfalsifiable.

### The existing sentinel test does not audit what its name claims

**COMPLECTED.** `no_durable_state_carries_a_sentinel_bearer_key` reads as the
privacy guarantee for the whole release. It actually asserts far less:

```rust
for path in [snapshot.workspaces.iter().flat_map(|w| {
    w.id().chars().chain(w.cloud_consent_at().unwrap_or("").chars())
})] {
```

That is a loop over a one-element array — clippy flags it as
`for loop over a single element` — collecting only the Workspace `id` and
consent timestamp. Those two fields are a UUID and an RFC3339 string; neither
could ever carry a bearer key. The test passes because it checks nothing that
could fail. The trailing `let _ = serialized;` discards the one value that
might have been interesting.

Meanwhile the issue's prohibited locations are "SQLite, archives, backups,
logs, errors, UI snapshots, process arguments, or generated artifacts" — none
of which this test opens. The test's *name* is doing the work its *body* does
not, which is exactly the failure mode a release audit exists to catch.

**Resolution:** replace the body with an audit that plants the sentinel through
the real keychain seam and then reads the actual bytes of the SQLite file, a
backup, an exported archive, and a Markdown export. Keep one honest carve-out,
stated in the assertion message rather than in a comment: a sentinel the thinker
*typed into a Note* is their text and is expected in Note-text columns. Every
other byte is a failure.

### "Network audit finds no telemetry, sync, updater, or hidden provider"

**CAUTION → resolvable, with one scope correction.** The Rust side is clean and
narrow: exactly four modules decide outbound traffic, `reqwest` is built with
`default-features = false, features = ["rustls"]`, the app registers no updater
plugin, and the webview CSP is `default-src 'self'`. An allowlist audit over
those modules is cheap and meaningful.

The repository as a whole is not clean, and this is where the issue's scope
fence needs a correction rather than obedience. Alongside the shipped Tauri app
the tree carries a complete, unreferenced Next.js prototype — `app/`,
`components/`, `lib/`, `proxy.ts`, `next.config.mjs` — containing:

- a Umami analytics script tag (`https://cloud.umami.is/script.js`) in
  `app/layout.tsx`, explicitly allowed by the prototype's own CSP;
- provider configuration for OpenRouter, OpenAI, and z.ai in
  `lib/ai-settings.ts`;
- a public CORS proxy route, `app/api/fetch-url/route.ts`, plus its rate
  limiter in `proxy.ts`.

None of it reaches the artifact: `tsconfig.json` includes only `src/**`, eslint
lints only `src`, and Vite's entry is `index.html → src/main.tsx`. So the
shipped `.app` is unaffected. But "no telemetry, analytics, or non-Ollama
provider traffic" is stated in this issue as a *decision*, not merely as an
artifact property, and a reader auditing this repository against that decision
finds analytics and three non-Ollama providers.

**Resolution (scope correction, owner-approved):** delete the two items that are
privacy violations with no forward use — the Umami script tag with its CSP
allowance, and the CORS proxy route with its rate limiter. Retain
`lib/ai-settings.ts` and the YouTube embed components: the owner intends
OpenRouter, OpenAI, and z.ai as fallback providers and YouTube embeds as
product surface in a **later** issue, and deleting the source material now would
only mean rewriting it later.

Scope the release audits to the shipped surface — `src/`, `src-tauri/`, and the
built `dist/` — and state in the release notes that the Next prototype is not
built, not bundled, and not shipped. This keeps #19 inside its fence (no
provider expansion here) while making its privacy claim true as written.

### Fresh install and durable recovery

**CLEAN.** `WorkspaceStore::open` against a nonexistent path already runs every
migration and seeds the initial Workspace, and the suite already covers reopen
recovery for notes, labels, FTS, relationships, and selection. The gap is only
that no single test walks the whole clean-app-data path end to end. One
integration test, not a new module.

### Apple Silicon artifact

**CLEAN, with two config defects.** `tauri.conf.json` sets
`bundle.targets: ["app"]`, so no DMG is produced, and `bundle.icon` is an empty
array while `src-tauri/icons/icon.icns` exists — the bundle ships without its
icon. Both are one-line config fixes, not design questions. The host is Apple
Silicon, so `tauri build` targets `aarch64-apple-darwin` natively.

Signing and notarization stay manual and must be *documented as absent* rather
than faked. This is the one acceptance criterion that is satisfied by an honest
negative statement.

---

## COMPLEXITY SCORECARD

State Surface: **None added** — this slice introduces no durable state and no
new command. The audits are tests; the release notes are prose.
Seam Quality: **Preserved and now enforced** — the four network modules and the
one keychain module already were the seams. The audits turn "we believe this is
the whole list" into a test that fails when the list grows.
Module Cohesion: **Cohesive** — one audit module owns both release rules.
Change Blast Radius: **Narrow** — a new outbound host edits one allowlist; a new
artifact writer edits one sentinel location list.
Incidental Complexity Load: **Mostly Problem** — proving a privacy claim is
intrinsic to shipping one. The one piece of genuinely incidental complexity is
the dead Next prototype, and it is being reduced rather than audited around.

Summary: The PRD is structurally sound and adds no product complexity. Two
findings modify it. First, the release gate it treats as a precondition does not
exist and has been failing unobserved — it must be built and repaired before any
acceptance criterion about it is meaningful. Second, the existing sentinel test
is a name without a body, and reusing it as-is would ship a privacy guarantee
that has never been checked. Both are repairs to the *verification*, not to the
product.

---

## GATE DECISION: PROCEED WITH MODIFICATIONS

Hand to implementation:

1. Repair `cargo fmt` and the 11 `cargo clippy` errors; wire both into
   `npm run check` so the release gate is one command.
2. Replace the sentinel test body with a real audit over SQLite bytes, backups,
   archives, and exports.
3. Add the network allowlist audit over `src/` and `src-tauri/`.
4. Add the clean-app-data end-to-end integration test.
5. Remove the Umami script tag, its CSP allowance, and the CORS proxy route.
   Retain `lib/ai-settings.ts` and the YouTube components for the follow-up.
6. Fix `bundle.targets` and `bundle.icon`; build and verify the Apple Silicon
   artifact opens and persists across restart.
7. Write `docs/v0/release-notes.md` with V0 scope, exclusions, the non-shipped
   status of the Next prototype, and the explicit signing/notarization status.

Defer to a follow-up issue: wiring OpenRouter, OpenAI, and z.ai as fallback
providers, and YouTube embeds, into the Tauri app. Both are provider/product
expansion and are fenced out of this release slice.

---

## POST-IMPLEMENTATION ADDENDUM

Recorded after `code-review`, because two findings change what this audit
concluded rather than merely adding detail.

### The sentinel audit repeated the exact failure it was written to fix

This audit's second finding was that the old test "is a name without a body."
The first replacement written for it **had the same defect in a subtler form**:
it read the bytes of every prohibited artifact, but nothing in the test ever
put `SENTINEL_KEY` anywhere. The value was only ever searched for, so every
assertion was vacuous — no plumbing change could have made it fail. The
positive control proved the scan could read bytes; it did not prove the secret
had ever entered the system.

The shipped version plants the sentinel in the keychain the production Cloud
path reads, drives a real `discover_models` call, and asserts through a
recording client that the key reached the bearer header — *then* scans. The
lesson generalises: a negative assertion is only as strong as the proof that
the thing being searched for was ever present.

### A real secret leak, missed by this audit entirely

Review found that `SecurityCliKeychain::write` passed the bearer key as
`security add-generic-password … -w <value>`, putting the secret in `argv`
where `ps` exposes it to any local process. The module's own doc comment
claimed the opposite — "passed through stdin … so it never appears in the
process's command line" — and had done since the secret seam was written.

This audit read that comment and believed it. It scored the keychain seam as
CLEAN without checking the code against its own documentation, and the
prohibited location "process arguments" was the one item in the issue's list
that no planned artifact scan could ever have reached. Fixed by writing the
value to the child's stdin, plus `secrets::process_argument_audit` to pin it.

Both corrections point the same way: **prefer a test that must first make the
bad thing happen over one that asserts it did not.**
