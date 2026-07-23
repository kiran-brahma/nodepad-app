# PRD SIMPLICITY AUDIT

Feature: V0-10 — Ollama Cloud consent, keychain, and discovery (GitHub issue #11)  
Date: 2026-07-23  
Gate: PROCEED

---

## MODULE MAP

- `src-tauri/src/ollama.rs` — local Ollama discovery; the `DiscoveryFailureCode` enum grows to admit the three cloud-only codes (`Unauthenticated`, `AuthenticationFailed`, `RateLimited`) so the UI has one failure type with one message per case. The `DiscoveryOutcome` shape is unchanged.
- `src-tauri/src/secrets.rs` (new) — the secret seam. `KeychainAdapter` is the contract; `SecurityCliKeychain` is the production `security`-CLI implementation; `fake::FakeKeychain` is the test fixture. The key is read on every cloud call and dropped at the end of that scope.
- `src-tauri/src/cloud.rs` (new) — `CloudOllamaProvider` is the cloud sibling of the local one. Same opaque identifiers, same response shape, same parser, but with bearer auth and a richer failure set. A `From` impl bridges the cloud outcome to the shared `DiscoveryOutcome` so the UI has one outcome type.
- `src-tauri/src/lib.rs` — wires the cloud provider and the keychain into `AppState`; adds five new Tauri commands (`set_cloud_consent`, `set_cloud_api_key`, `delete_cloud_api_key`, `cloud_api_key_present`, `discover_cloud_models`). Storage of consent is delegated to the existing `WorkspaceStore`; the keychain is the only durable holder of the key.
- `src-tauri/migrations/0007_cloud_consent.sql` (new) — adds `cloud_consent_at TEXT` to `thinking_workspaces`. The bearer key column is not in the schema at all.
- `src-tauri/src/workspace.rs` — `ThinkingWorkspace` gains `cloud_consent_at: Option<String>`. `ThinkingWorkspaceInterface` gains `set_cloud_consent(workspace_id, accept)`. The `MemoryStore` and the SQLite `WorkspaceStore` both implement it. The accessor `cloud_consent_at()` and `WorkspaceSnapshot::workspaces()` are the only new public surfaces.
- `src/workspace-client.ts` — `ThinkingWorkspace` gains `cloudConsentAt`. `DiscoveryFailureCode` and `DiscoveryState` are unified here and re-exported so both hooks import the one shape. New commands: `setCloudConsent`, `setCloudApiKey`, `deleteCloudApiKey`, `cloudApiKeyPresent`, `discoverCloudModels`. New type: `CloudSecretOutcome`.
- `src/use-cloud-discovery.ts` (new) — sibling of `use-local-discovery.ts`. Same request-counter cancellation; clears state when the Workspace is not on Cloud AI or has not consented. Never reads the key.
- `src/cloud-consent-dialog.tsx` (new) — the disclosure a Workspace must accept before Cloud AI is usable. Records consent only on accept; closing without accept writes nothing.
- `src/cloud-key-section.tsx` (new) — the form for entering, replacing, and removing the key. The key is sent once to the backend and dropped from React state as soon as the call resolves.
- `src/assistance-section.tsx` — gains a Cloud AI branch. The policy buttons are unchanged; selecting Cloud AI on a non-consented Workspace opens the dialog instead of writing the policy.
- `src/App.tsx` — owns the consent dialog state. Selecting Cloud AI without consent opens the dialog; accepting commits consent and then sets the policy. Revoking consent sets the policy to Manual so the durable state can never read "cloud_ai" on an unconsented Workspace.
- `src/styles.css` — minor additions for the cloud key section. The dialog uses existing form styling.

The blocker (#10) is merged into `main` as `5977630` (V0-09 Assistance Policy and local Ollama discovery). The local code path is untouched: the local provider, the `TagsClient` trait, the local failure codes, and the local hook all keep their original shapes.

---

## INTERROGATION FINDINGS

### The shared outcome type — COMPLECTED, resolved

The issue says "Authentication failure, timeout, rate limit, malformed response, empty list, and retired/missing selected model are distinct." The local and cloud providers could each return a separate outcome type with its own failure code, but the UI's discovery surface (search/refresh/select/missing) would then need to switch on which provider produced the outcome, and the test fixtures would need a parallel set of `committed` and `failed` shapes. A thought experiment: if local and cloud each had their own `DiscoveryOutcome`, the `useLocalDiscovery` and `useCloudDiscovery` hooks would each have a `state.kind === "ready"` and `state.kind === "error"` branch, and the `assistance-section.tsx` would dispatch on policy first and then on outcome variant — three places reasoning about the same shape.

The resolution is one outcome type with one set of failure codes. The local provider only ever produces the four codes it already had; the cloud provider also produces the three cloud-only codes (`Unauthenticated`, `AuthenticationFailed`, `RateLimited`). `cloud.rs` provides a `From<CloudDiscoveryOutcome> for DiscoveryOutcome` that maps the cloud-specific code to the same enum variant, so the UI reads one set of cases. The test fixtures show this is not just a Rust-side convenience: the same `DiscoveryOutcome` shape is what the React hook hands to the `assistance-section.tsx` error renderer.

### Consent and the durable policy — CAUTION, resolved

Cloud AI cannot activate until the disclosure is accepted. Two readings of "cannot activate": the UI must not show the cloud branch without consent, and the durable policy must not read `cloud_ai` on a Workspace that has not consented. The first is straightforward — the hook only fetches when `cloudConsentAt` is set. The second is a question of ordering: if the UI commits `setAssistancePolicy(workspace_id, "cloud_ai")` and then commits `setCloudConsent(workspace_id, true)`, a crash between the two leaves the Workspace on cloud_ai without consent, and the next read will return the inconsistent pair.

The resolution is to invert the order. `set_cloud_consent` is its own command, separate from `set_assistance_policy`. The UI's "Cloud AI" button only opens the disclosure if the Workspace is not consented; the disclosure's accept button commits consent and only then does the parent commit the policy. Revoke does the same in reverse (consent cleared, policy set to manual). A failed commit in either step leaves the other step's effect durable, so the worst case is "consent granted but policy not yet moved" — which the next click handles, and never "policy moved without consent."

### The key as a one-shot value — CLEAN

The issue says the key may not be written to SQLite, browser storage, React persistence, logs, errors, archives, crash output, command-line arguments, or test snapshots. The key is read from the keychain on every cloud call, held in a `String` for the duration of one `discover_models` call, and dropped at the end of that scope. It never enters the database, the React state, the serialized snapshot, the dev tools, or a Tauri command's return value. The `setCloudApiKey` and `deleteCloudApiKey` commands return a typed outcome with no payload; `cloudApiKeyPresent` returns a `bool`. The form clears its draft as soon as the call resolves, so the React state loses the value before the next render.

The "scan serialized state for a sentinel key" tests at the Rust and TypeScript layers encode this rule: a sentinel value written to a Note text or to a model name is not asserted-against, but the snapshot's JSON and the SQLite row dump are both scanned for the sentinel and must not contain it. A future change that quietly writes the key to, say, the active Workspace's name will fail the test, not the reviewer.

### The keychain as a single app-level credential — CLEAN

The issue says "One application-level Ollama Cloud credential may be used by multiple consented Workspaces; consent remains per Workspace." The keychain is keyed on a fixed service-and-account pair (`com.nodepad.desktop` / `ollama-cloud-bearer`), so all Workspaces share one stored key. Consent is per Workspace, on the database row. Splitting this into a per-Workspace keychain item would be wasted complexity: the issue says the credential is application-level, and the keychain becomes a single point of revocation. Removing the credential is a one-click operation; affected Workspaces surface the absence through the typed `Unauthenticated` failure on the next cloud call.

### Switching away invalidates pending work — CLEAN

Switching the Assistance Policy to Manual or Local, or revoking consent, must invalidate outstanding cloud requests. The hook's request counter drops responses that arrive after the policy changes, the same rule `use-local-discovery.ts` already has. The provider's `discover_models` call resolves against the keychain it read at the start of the call, so a revoked consent or removed key partway through a request still produces one outcome — but that outcome arrives after the hook has cleared its state, so the user never sees it. The next call from a non-cloud Workspace takes the early-out path in `discover_cloud_models` (consent not granted), so no extra HTTP request is made.

### Disclosure text — CAUTION, deviating deliberately

The issue does not specify the disclosure text. The chosen text covers the four things the spec lists (Note content leaves the Mac, bounded context leaves the Mac, the key is in the keychain, the choice is per Workspace and revocable) and the two things a reasonable reader would want to know (that the bearer key is not stored anywhere else, and that other Workspaces are not affected). It does not list the specific model identifiers that will be sent, because the spec is explicit that this slice does model discovery only and sends no Note content. Adding "and here is what the request body will look like" is a future slice's job; baking the format into a UI string would lock it in.

---

## COMPLEXITY SCORECARD

State Surface: Low — one new per-Workspace column (`cloud_consent_at`), one new keychain entry (shared), no in-memory state the UI holds between renders.  
Seam Quality: Improved — the keychain and the cloud HTTP client are both traits with one production impl and one fake, mirroring `TagsClient`. The local path is untouched.  
Module Cohesion: Cohesive — secret seam in `secrets.rs`, cloud provider in `cloud.rs`, consent on the Workspace, key UI in `cloud-key-section.tsx`, disclosure in `cloud-consent-dialog.tsx`.  
Change Blast Radius: Narrow — three new Rust modules, one new migration, one new hook, one new dialog, one new section component, one App-level wiring change, no edits to the durable interface beyond the new column and the new outcome methods.  
Incidental Complexity Load: Mostly Problem

Summary: The feature is a third Assistance Policy value backed by a new HTTP host and a new secret store. The one real risk — leaking the bearer key into durable state, logs, or the React tree — is removed by construction: the keychain adapter is the only thing that holds the key, and it is read on every call. The other risk — the UI's policy and the Workspace's consent drifting out of sync — is removed by ordering the commits so consent precedes policy. The local provider, the local hook, and the local failure codes are untouched.

---

## GATE DECISION: PROCEED

### Implementation constraints carried forward

1. `DiscoveryOutcome` is the single shape the UI reads. Local and cloud providers both return it; the cloud-only failure codes are new variants on the shared `DiscoveryFailureCode` enum.
2. `set_cloud_consent` is its own command, separate from `set_assistance_policy`. The UI commits consent before the policy and revokes before returning to manual, so a failure on one step never leaves the other half inconsistent.
3. The bearer key is read from the keychain on every cloud call, held for the duration of one HTTP request, and dropped at the end of that scope. It never appears in the database, the snapshot, the React tree, a log line, or a command return value.
4. The keychain adapter is a trait; production uses `SecurityCliKeychain` (the macOS `security` CLI), tests use `FakeKeychain`. The contract is the only thing under test.
5. The hook's request counter drops cloud responses that arrive after the policy changes, mirroring `use-local-discovery.ts`. A revoked consent or removed key partway through a request never reaches the UI.
6. Tests scan serialized state and database rows for a sentinel bearer key, and assert the key never appears there. The same sentinel is asserted absent from the React snapshot in the frontend tests.
7. Local discovery and the local provider are unchanged. The local `TagsClient` trait, the local failure codes, the local hook, and the local UI branch all keep their original shapes.
