# Nodepad V0 — release notes

Version 0.1.0 · Apple Silicon macOS (`aarch64-apple-darwin`) · macOS 11 or later

Nodepad is a personal thinking tool for developing a topic spatially and
associatively before writing. V0 supports thinking and export. Long-form
composition is deliberately not in this release.

---

## What V0 supports

**Thinking Workspaces.** Create, rename, delete, and switch between
self-contained maps of one topic. A fresh install seeds one Workspace and
opens it.

**Notes.** Capture, edit, pin, delete, move, and copy atomic thoughts. Each
Note carries a Note Type (claim, question, idea, reference, thesis), any number
of Labels, and an optional Annotation.

**Relationships and the Thinking Graph.** Relate any two Notes in the same
Workspace. Relationships are symmetric and untyped in V0.

**Views.** Tiling, Kanban, and a graph view over the same committed Notes.

**Search.** Full-text search across Note text, Note Types, and Labels.

**Export.** Markdown export for one Workspace, and a `.nodepad` JSON archive
that round-trips a Workspace into any install.

**Backup and restore.** One automatic backup per day when data has changed,
plus guarded pre-migration and pre-restore backups. Every backup is checksummed
and verified before it can be restored.

**Undo.** Command-Z compensates the last change in the active Workspace. Undo
history is per-session and deliberately does not survive a restart.

**AI Assistance (optional).** Per-Workspace Assistance Policy: Manual, Local
AI, or Cloud AI. AI can suggest Labels, Annotations, Relationships, and
Syntheses. Every assisted result stays editable, and every Workspace is fully
usable with AI switched off.

**macOS behaviour.** Full keyboard access, a Command-K palette, coordinated
Escape, focus trapping in modals, visible focus rings, reduced-motion support,
and external links opened in the default browser.

---

## Privacy and network behaviour

Nodepad V0 contacts exactly three kinds of destination, and nothing else:

| Destination | When |
| --- | --- |
| `http://localhost:11434` | Only under the Local AI policy — the thinker's own Ollama host |
| `https://ollama.com` | Only under the Cloud AI policy, and only after explicit per-Workspace consent |
| A URL written in a Note | Only for metadata on an explicit HTTP(S) link, and only after the address resolves to a public host |

Links a thinker clicks are handed to the macOS opener and open in the default
browser. The webview itself never navigates, and only `http` and `https` are
accepted — `mailto`, `tel`, `file`, `javascript`, and custom app schemes are
refused.

**Nodepad V0 has no telemetry, no analytics, no crash or error reporting, no
cloud sync, no account system, and no background update check.** There is no
non-Ollama AI provider in this build.

**Manual mode is fully offline.** With the network disabled and no credential
in the keychain, every feature above except AI Assistance works normally.

**The Ollama Cloud key lives only in the macOS keychain.** It is read at the
moment of a request, sent as a bearer token, and never written to the database,
a backup, an archive, an export, a log line, an error message, or a process
argument.

The process-argument half of that was not true before this release: the
keychain write passed the key as `security … -w <value>`, which put it in this
process's `argv` where `ps` exposes it to any local process for the duration of
the call. It is now written to the child's stdin instead, and an audit fails
the build if it ever returns to the command line.

### How these claims are enforced

These are not documentation-only promises. `src-tauri/src/release_audit.rs`
fails the build when any of them stops being true:

- `every_outbound_origin_is_on_the_release_allowlist` — reads the shipped
  source and rejects any HTTP(S) origin not on the table above.
- `no_telemetry_analytics_updater_or_hidden_provider_is_present` — rejects
  known analytics, error-reporting, updater, and non-Ollama provider markers.
- `no_dependency_introduces_telemetry_or_an_updater` — applies the same rule to
  the Rust dependency list.
- `the_webview_makes_no_network_call_of_its_own` — the webview reaches the
  network only through a typed Tauri command.
- `the_webview_content_security_policy_denies_every_remote_origin` — the CSP
  is `default-src 'self'` with no remote origin admitted.
- `only_the_reviewed_tauri_plugins_are_registered` — the file dialog is the
  only registered plugin.
- `the_built_front_end_carries_no_forbidden_marker_or_unapproved_origin` —
  applies the same two rules to the *built* `dist/` bundle, where minification
  and a dependency's runtime can introduce a string that appears in no source
  file.
- `the_embedded_development_url_is_loopback_and_unused_in_release` — pins the
  one development URL that ships (see below) to a loopback address.
- `sentinel::a_bearer_key_in_flight_reaches_the_request_and_no_durable_byte` —
  puts a sentinel key in the keychain the production Cloud path reads, drives a
  real Cloud discovery, and asserts via a recording client that the key
  genuinely reached the bearer header. Only then does it read the raw bytes of
  the SQLite file and its write-ahead log, a backup, an exported archive, a
  Markdown export, the UI snapshot, the discovery outcome, the recorded
  keychain calls, and an error message, and require the sentinel in none of
  them. It also carries a positive control — text the thinker typed, which
  *must* be found — so it fails loudly rather than passing vacuously if the
  scan ever stops reading those bytes.
- `secrets::process_argument_audit` — process arguments are the one prohibited
  location a durable-byte scan cannot reach, so they are audited next to the
  code that could leak them. The keychain write passes the key on the child's
  stdin; the audit fails if it is ever handed to the command line, where `ps`
  would expose it to any local process.

---

## Platform support

V0 supports **Apple Silicon macOS only**, macOS 11 or later.

Intel macOS, Windows, Linux, and the browser are **not supported** and are not
built, tested, or shipped. Nodepad is not distributed through the App Store.

### The Next.js prototype in this repository is not shipped

The repository also carries an unreferenced Next.js prototype under `app/`,
`components/`, and `lib/`. It is **not compiled, not bundled, and not part of
any release artifact**: `tsconfig.json` includes only `src/**`, eslint lints
only `src`, and the Vite entry point is `index.html → src/main.tsx`.

It is retained as source material for planned work on additional AI providers
and embedded video. Its Umami analytics script and its public CORS proxy route
were removed in this release; nothing in it reaches a Nodepad build.

---

## Signing and notarization status

**This build is unsigned and un-notarized.**

The artifact carries only an ad-hoc signature applied automatically by the
linker (`Signature=adhoc`). No Apple Developer ID was used, because no signing
credentials are available to this project. This is the only reason these steps
remain manual — they are not skipped for convenience, and nothing about the
build prevents them.

Gatekeeper will therefore refuse the app on first open. To run it:

```bash
xattr -dr com.apple.quarantine /Applications/Nodepad.app
```

Or open it once via Finder's right-click → Open.

### To sign and notarize a build

These commands are documented, not run. They require credentials this project
does not have; do not treat the placeholders as real values.

Signing needs a "Developer ID Application" certificate in the login keychain,
and Tauri reads the identity from the environment:

```bash
APPLE_SIGNING_IDENTITY="Developer ID Application: YOUR NAME (TEAMID)" npx tauri build --target aarch64-apple-darwin
```

Notarization additionally needs an App Store Connect API key, or an
app-specific password:

```bash
APPLE_SIGNING_IDENTITY="Developer ID Application: YOUR NAME (TEAMID)" APPLE_ID="you@example.com" APPLE_PASSWORD="app-specific-password" APPLE_TEAM_ID="TEAMID" npx tauri build --target aarch64-apple-darwin
```

Required secrets, none of which exist today:

| Secret | What it is |
| --- | --- |
| `APPLE_SIGNING_IDENTITY` | The Developer ID Application certificate name |
| `APPLE_ID` | The Apple ID that owns the developer account |
| `APPLE_PASSWORD` | An app-specific password for that Apple ID |
| `APPLE_TEAM_ID` | The 10-character developer team identifier |

After notarization, staple the ticket so the app validates offline:

```bash
xcrun stapler staple src-tauri/target/aarch64-apple-darwin/release/bundle/macos/Nodepad.app
```

---

## Building and verifying

The complete release gate is one command, and nothing ships past a failure in
it:

```bash
npm run check
```

It runs TypeScript, eslint, the Vitest UI and integration suites,
`cargo fmt --check`, `cargo clippy -D warnings`, the Rust test suite including
both release audits, the production front-end build, and the packaging smoke.

To produce the Apple Silicon artifact:

```bash
npx tauri build --target aarch64-apple-darwin
```

This writes `Nodepad.app` and `Nodepad_0.1.0_aarch64.dmg` under
`src-tauri/target/aarch64-apple-darwin/release/bundle/`.

### One note on the artifact's contents

The embedded Tauri configuration contains the development server address
`http://127.0.0.1:1420`. It is inert in a release build: the release loads its
front end from the bundled assets, the CSP admits no remote origin, and the
address is loopback rather than a remote endpoint. It is recorded here rather
than left for a reader to find in a strings dump.

---

## Known limitations

- Undo history does not survive a restart, by design.
- Relationships are symmetric and untyped.
- Synthesis is rate-limited and needs a minimum number of organized Notes.
- URL metadata is fetched only for explicit HTTP(S) links that resolve to a
  public address; private and link-local addresses are refused.
- Ollama Cloud shares one keychain credential across all consented Workspaces.
