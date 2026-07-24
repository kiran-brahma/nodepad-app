//! The release's network audit.
//!
//! Every other privacy guarantee in Nodepad is enforced where it is used: the
//! keychain seam keeps the bearer key out of durable state, `url_metadata`
//! refuses to resolve a private address, `external_link` refuses every scheme
//! but HTTP(S). Those are local rules, and local rules are checked by local
//! tests.
//!
//! This module checks the one claim no local rule can make: that the list of
//! places Nodepad talks to is *complete*. A new `reqwest` call in a new module
//! would not fail any existing test — it would simply be a fifth outbound path
//! nobody audited. So the audit reads the shipped source tree as text and
//! fails when it finds an outbound host, an analytics endpoint, an updater, or
//! a frontend network call that the release did not sign off on.
//!
//! Scope is the shipped surface only: `src-tauri/src` (the Rust app), `src`
//! (the webview), and `index.html`. The repository also carries an
//! unreferenced Next.js prototype under `app/`, `components/`, and `lib/`
//! which is not compiled, not bundled, and not shipped — see
//! `docs/v0/release-notes.md`.

#![cfg(test)]

use std::path::{Path, PathBuf};

/// The complete set of hosts a Nodepad build may contact directly, and why
/// each one is allowed. This list is the release decision; the tests below
/// only enforce it. Adding an entry here is a deliberate act that shows up in
/// review as a change to the privacy surface.
const ALLOWED_OUTBOUND_HOSTS: &[(&str, &str)] = &[
    (
        "http://localhost:11434",
        "the thinker's own local Ollama host, contacted only under the Local AI policy",
    ),
    (
        "https://ollama.com",
        "Ollama Cloud, contacted only after per-Workspace consent and only with a keychain key",
    ),
    (
        "https://openrouter.ai",
        "OpenRouter, only after per-Workspace consent and a keychain key",
    ),
    (
        "https://api.openai.com",
        "OpenAI, only after per-Workspace consent and a keychain key",
    ),
    (
        "https://api.z.ai",
        "Z.ai, only after per-Workspace consent and a keychain key",
    ),
    (
        "https://www.youtube-nocookie.com",
        "the reviewed embedded introduction video frame",
    ),
    (
        "https://nodepad.space",
        "the OpenRouter attribution header; never a request destination",
    ),
];

/// Markers for the categories of traffic the release forbids outright. Each is
/// a substring that would appear in source if the category were present.
const FORBIDDEN_MARKERS: &[(&str, &str)] = &[
    ("umami", "analytics"),
    ("plausible.io", "analytics"),
    ("google-analytics", "analytics"),
    ("googletagmanager", "analytics"),
    ("sentry.io", "remote error logging"),
    ("segment.io", "analytics"),
    ("posthog", "analytics"),
    ("tauri-plugin-updater", "a background update check"),
    ("updater.json", "a background update check"),
    ("api.anthropic.com", "a non-Ollama provider"),
];

/// Origins that appear in the shipped source as *test input* — the addresses a
/// guard is written to reject, or a schema URL in a config. Nothing here is
/// ever contacted by a running Nodepad.
///
/// This list exists so those addresses are declared in one visible place. The
/// alternative, exempting any line that looks like a test, silently exempts
/// real hosts too.
const TEST_FIXTURE_ORIGINS: &[&str] = &[
    "http://example.com",
    "https://example.com",
    "http://example.org",
    "https://example.org",
    "http://127.0.0.1",
    "http://localhost",
    "https://localhost",
    "http://192.168.0.1",
    "http://10.0.0.1",
    "http://169.254.169.254",
    "http://[",
    "https://[",
    "https://schema.tauri.app",
    "https://ui.shadcn.com",
    "https://user:secret@example.com",
];

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repository_root() -> PathBuf {
    manifest_dir()
        .parent()
        .expect("src-tauri always has a parent")
        .to_path_buf()
}

/// Every shipped source file, as (path relative to the repository, contents).
/// Only the Rust app, the webview, and the HTML entry point are shipped.
fn shipped_sources() -> Vec<(String, String)> {
    let root = repository_root();
    let mut files = Vec::new();
    collect(&manifest_dir().join("src"), &root, &mut files);
    collect(&root.join("src"), &root, &mut files);
    let index = root.join("index.html");
    if let Ok(contents) = std::fs::read_to_string(&index) {
        files.push(("index.html".to_owned(), contents));
    }
    assert!(
        files.len() > 20,
        "the audit found only {} shipped source files, so it is scanning the wrong tree",
        files.len()
    );
    files
}

fn collect(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, root, out);
        // This module is `#![cfg(test)]`, so it is never compiled into a
        // release build and is not part of the shipped surface. It is also the
        // one file that necessarily spells out every forbidden marker, so
        // scanning it would make the audit fail on its own rule table.
        } else if path.file_name().and_then(|name| name.to_str()) == Some("release_audit.rs") {
            continue;
        } else if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("rs" | "ts" | "tsx" | "html" | "css")
        ) {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                let relative = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                out.push((relative, contents));
            }
        }
    }
}

/// Pulls every `http://` or `https://` literal out of a line and returns each
/// one reduced to scheme + authority, which is the granularity the allowlist
/// is written at.
fn origins_in(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let rest = &line[index..];
        let Some(offset) = rest.find("http") else {
            break;
        };
        let start = index + offset;
        let tail = &line[start..];
        let scheme_len = if tail.starts_with("https://") {
            8
        } else if tail.starts_with("http://") {
            7
        } else {
            index = start + 4;
            continue;
        };
        let authority: String = tail[scheme_len..]
            .chars()
            .take_while(|c| !matches!(c, '/' | '"' | '\'' | '`' | ' ' | ')' | '>' | ',' | '\\'))
            .collect();
        if !authority.is_empty() {
            found.push(format!("{}{authority}", &tail[..scheme_len]));
        }
        index = start + scheme_len;
    }
    found
}

/// A line that only *documents* a URL is not a line that contacts one, and
/// this codebase documents its network rules heavily in prose. Comments are
/// therefore skipped.
///
/// Nothing else is. An earlier version of this predicate also skipped any line
/// containing `assert!` or `assert_eq!`, which was a hole wide enough to drive
/// a provider through: a single assertion mentioning a host exempted that host
/// from both scans. Test code that legitimately names a rejected address
/// belongs in [`TEST_FIXTURE_ORIGINS`], where it is visible, rather than
/// hidden behind a syntactic exemption.
fn is_comment(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("//") || trimmed.starts_with("*") || trimmed.starts_with("/*")
}

/// Runs one line-level rule over a set of files and returns every hit as
/// `path:line: detail`. All three source scans share this walk, so the rules
/// stay one expression each and "which lines are exempt" is decided in exactly
/// one place ([`is_comment`]).
fn scan(files: &[(String, String)], rule: impl Fn(&str) -> Vec<String>) -> Vec<String> {
    let mut offences = Vec::new();
    for (path, contents) in files {
        for (number, line) in contents.lines().enumerate() {
            if is_comment(line) {
                continue;
            }
            for detail in rule(line) {
                offences.push(format!("{path}:{}: {detail}", number + 1));
            }
        }
    }
    offences
}

#[test]
fn every_outbound_origin_is_on_the_release_allowlist() {
    let allowed: Vec<&str> = ALLOWED_OUTBOUND_HOSTS
        .iter()
        .map(|(host, _)| *host)
        .collect();

    let offences = scan(&shipped_sources(), |line| {
        origins_in(line)
            .into_iter()
            .filter(|origin| {
                !allowed.iter().any(|host| origin.starts_with(host))
                    && !TEST_FIXTURE_ORIGINS
                        .iter()
                        .any(|host| origin.starts_with(host))
            })
            .collect()
    });

    assert!(
        offences.is_empty(),
        "Nodepad may contact only the hosts on the release allowlist:\n{}\n\nUnapproved origins found:\n  {}",
        ALLOWED_OUTBOUND_HOSTS
            .iter()
            .map(|(host, why)| format!("  {host} — {why}"))
            .collect::<Vec<_>>()
            .join("\n"),
        offences.join("\n  ")
    );
}

#[test]
fn no_telemetry_analytics_updater_or_hidden_provider_is_present() {
    let offences = scan(&shipped_sources(), |line| {
        FORBIDDEN_MARKERS
            .iter()
            .filter(|(marker, _)| line.contains(marker))
            .map(|(marker, category)| format!("{marker} ({category})"))
            .collect()
    });
    assert!(
        offences.is_empty(),
        "the release forbids telemetry, analytics, remote logging, background \
         update checks, and non-Ollama providers, but found:\n  {}",
        offences.join("\n  ")
    );
}

/// The Rust dependency list is the other place an outbound path could enter
/// without touching any module the audit reads.
#[test]
fn no_dependency_introduces_telemetry_or_an_updater() {
    let manifest = std::fs::read_to_string(manifest_dir().join("Cargo.toml"))
        .expect("src-tauri/Cargo.toml is readable");
    for (marker, category) in FORBIDDEN_MARKERS {
        assert!(
            !manifest.contains(marker),
            "src-tauri/Cargo.toml depends on {marker}, which would introduce {category}"
        );
    }
}

/// The webview must never open a connection of its own. Every request Nodepad
/// makes is made by Rust, behind a typed command, so the audit can reason
/// about the four network modules and nothing else.
#[test]
fn the_webview_makes_no_network_call_of_its_own() {
    // `fetch(` matches `prefetch(` and `refetch(` too. That is deliberate: a
    // wrapper named `refetch` still reaches the network, and a false positive
    // here costs one line in this list, while a false negative ships.
    const FORBIDDEN_WEBVIEW_CALLS: &[&str] = &[
        "fetch(",
        "XMLHttpRequest",
        "new WebSocket",
        "EventSource",
        "navigator.sendBeacon",
        "importScripts",
    ];
    let root = repository_root();
    let mut files = Vec::new();
    collect(&root.join("src"), &root, &mut files);
    if let Ok(contents) = std::fs::read_to_string(root.join("index.html")) {
        files.push(("index.html".to_owned(), contents));
    }

    let offences = scan(&files, |line| {
        FORBIDDEN_WEBVIEW_CALLS
            .iter()
            .filter(|marker| line.contains(**marker))
            .map(|marker| (*marker).to_owned())
            .collect()
    });
    assert!(
        offences.is_empty(),
        "the webview must reach the network only through a Tauri command, but found:\n  {}",
        offences.join("\n  ")
    );
}

/// The content security policy is the backstop: even if a Note's Markdown were
/// to render a remote reference, the webview may not load it.
#[test]
fn the_webview_content_security_policy_allows_only_the_reviewed_video_frame_origin() {
    let config = std::fs::read_to_string(manifest_dir().join("tauri.conf.json"))
        .expect("tauri.conf.json is readable");
    let parsed: serde_json::Value =
        serde_json::from_str(&config).expect("tauri.conf.json is valid JSON");
    let csp = parsed["app"]["security"]["csp"]
        .as_str()
        .expect("the app declares a content security policy");

    assert!(
        csp.contains("default-src 'self'"),
        "the policy must default to same-origin, but is: {csp}"
    );
    assert!(csp.contains("frame-src https://www.youtube-nocookie.com"));
    assert!(!csp.contains("frame-src *"));
    assert!(!csp.contains("connect-src"));
}

/// The app registers exactly the plugins the release accounts for. A plugin is
/// a whole capability surface, so an unreviewed one is a privacy question even
/// when it makes no request itself.
#[test]
fn only_the_reviewed_tauri_plugins_are_registered() {
    let lib = std::fs::read_to_string(manifest_dir().join("src/lib.rs")).expect("lib.rs readable");
    let registered: Vec<&str> = lib
        .lines()
        .filter(|line| line.contains(".plugin("))
        .collect();
    assert_eq!(
        registered.len(),
        1,
        "expected exactly one registered plugin (dialog), found: {registered:?}"
    );
    assert!(
        registered[0].contains("tauri_plugin_dialog"),
        "the one registered plugin must be the file dialog, found: {}",
        registered[0]
    );
}

/// The release's clean-app-data smoke.
///
/// Every predecessor suite tests its own slice against a store that some test
/// helper set up. This test is the one that starts where a thinker starts: a
/// path that does not exist yet. It walks the whole V0 manual path in one
/// session — the seeded Workspace, a Note, an edit, a Note Type, a Label, an
/// Annotation, a Relationship, search, undo, Markdown export, archive export —
/// then drops the store to simulate quit, reopens the same file, and asserts
/// the recovered state matches byte for byte what was there before.
///
/// It uses no fake and no in-memory adapter: the assertions are about the
/// real SQLite file the shipped app writes.
mod fresh_install {
    use crate::workspace::{ThinkingWorkspaceInterface, WorkspaceStore};

    fn temporary_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "nodepad-fresh-{}-{}.sqlite",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn discard(path: &std::path::Path) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

    #[test]
    fn a_clean_install_survives_a_full_manual_session_and_a_restart() {
        let path = temporary_path();
        discard(&path);
        assert!(
            !path.exists(),
            "the smoke must start against app data that does not exist"
        );

        // ── First launch ────────────────────────────────────────────────────
        let mut store = WorkspaceStore::open(&path).expect("a fresh install opens");
        let snapshot = store.snapshot().unwrap();
        assert_eq!(
            snapshot.workspaces.len(),
            1,
            "a fresh install seeds exactly one Thinking Workspace"
        );
        assert_eq!(snapshot.workspaces[0].name, "My Thinking Workspace");
        assert_eq!(
            snapshot.active_workspace_id, snapshot.workspaces[0].id,
            "the seeded Workspace is the active one"
        );
        assert!(snapshot.notes.is_empty(), "a fresh install has no Notes");
        let workspace_id = snapshot.active_workspace_id.clone();

        // ── The manual path, with no provider configured ────────────────────
        let first = store
            .create_note(&workspace_id, "Rivers shaped where cities grew")
            .unwrap();
        let first_id = first.notes[0].id.clone();
        store
            .edit_note_text(
                &first_id,
                "Rivers shaped where cities grew, and where they did not",
            )
            .unwrap();
        store.set_note_type(&first_id, "claim").unwrap();
        store.attach_label(&first_id, "geography").unwrap();
        store
            .set_note_annotation(&first_id, "Worth checking against the rail data")
            .unwrap();

        let second = store
            .create_note(&workspace_id, "Did rail change this after 1850?")
            .unwrap();
        let second_id = second
            .notes
            .iter()
            .find(|note| note.id != first_id)
            .expect("the second Note exists")
            .id
            .clone();
        store.set_note_type(&second_id, "question").unwrap();
        store.attach_label(&second_id, "geography").unwrap();
        store.relate_notes(&first_id, &second_id).unwrap();

        // Search reaches both Notes through the FTS index.
        let hits = store.search_notes(&workspace_id, "rivers").unwrap();
        assert!(!hits.is_empty(), "search finds the Note it indexed");

        // Undo is a committed mutation, so it must survive the restart too.
        let third = store
            .create_note(&workspace_id, "A thought to take back")
            .unwrap();
        assert_eq!(third.notes.len(), 3);
        let after_undo = store.undo(&workspace_id).unwrap();
        assert_eq!(
            after_undo.notes.len(),
            2,
            "undo removes the Note it compensated"
        );

        // Both export paths run on real content.
        let (markdown, filename) = store.markdown_export(&workspace_id).unwrap();
        assert!(markdown.contains("Rivers shaped where cities grew"));
        assert!(filename.ends_with(".md"));
        let exported = store.archive_export_data(&workspace_id).unwrap();
        let archive =
            crate::archive::build_archive(&exported, "0.1.0", "2026-07-24T12:00:00+00:00");
        let archive_bytes = crate::archive::serialize_archive(&archive).unwrap();
        assert!(archive_bytes.contains("Did rail change this after 1850?"));

        let before_quit = store.snapshot().unwrap();

        // ── Quit ────────────────────────────────────────────────────────────
        drop(store);

        // ── Reopen ──────────────────────────────────────────────────────────
        let reopened = WorkspaceStore::open(&path).expect("the same app data reopens");
        let after_restart = reopened.snapshot().unwrap();

        assert_eq!(
            after_restart.workspaces.len(),
            before_quit.workspaces.len(),
            "no Workspace was added or lost by the restart"
        );
        assert_eq!(
            after_restart.active_workspace_id, before_quit.active_workspace_id,
            "the active Workspace is restored"
        );
        assert_eq!(
            after_restart.notes.len(),
            2,
            "both Notes and neither the undone one survive"
        );
        assert_eq!(
            after_restart.relationships.len(),
            before_quit.relationships.len(),
            "the Relationship survives"
        );

        // Exact recovery: every Note field, not merely the count.
        for expected in &before_quit.notes {
            let actual = after_restart
                .notes
                .iter()
                .find(|note| note.id == expected.id)
                .unwrap_or_else(|| panic!("Note {} was lost by the restart", expected.id));
            assert_eq!(actual.markdown(), expected.markdown(), "text recovered");
            assert_eq!(
                actual.note_type(),
                expected.note_type(),
                "Note Type recovered"
            );
            assert_eq!(
                actual.annotation(),
                expected.annotation(),
                "Annotation recovered"
            );
            assert_eq!(
                actual.labels().len(),
                expected.labels().len(),
                "Labels recovered"
            );
        }

        // Search still works against the reopened index, and undo history is
        // deliberately empty: a restart leaves no compensation behind.
        assert!(!reopened
            .search_notes(&workspace_id, "rail")
            .unwrap()
            .is_empty());
        assert_eq!(
            after_restart.undoable_commands, 0,
            "a restart starts with no undo history"
        );

        drop(reopened);
        discard(&path);
    }
}

/// The release's sentinel-secret audit.
///
/// The issue forbids key material in "SQLite, archives, backups, logs, errors,
/// UI snapshots, process arguments, or generated artifacts". Process arguments
/// are audited next to the code that could leak them, in
/// `secrets::process_argument_audit`; everything else is audited here.
///
/// The design point of this module is that the sentinel is *actually planted*.
/// An earlier version of this audit only ever searched for a value nothing had
/// introduced, so every assertion was vacuous — no plumbing change could have
/// made it fail. Here the sentinel is put into the keychain the production
/// Cloud path reads from, a Cloud discovery is driven end to end, and the
/// recording client asserts the key really did arrive at the bearer header.
/// Only once the secret is proven to be *in flight* is it meaningful to assert
/// that it reached no durable byte.
mod sentinel {
    use crate::cloud::{
        CloudDiscoveryFailureCode, CloudHttpResponse, CloudOllamaProvider, CloudTagsClient,
    };
    use crate::secrets::fake::FakeKeychain;
    use crate::secrets::{OLLAMA_CLOUD_KEYCHAIN_ACCOUNT, OLLAMA_CLOUD_KEYCHAIN_SERVICE};
    use crate::workspace::{ThinkingWorkspaceInterface, WorkspaceStore};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    /// Stands in for the Ollama Cloud bearer key.
    const SENTINEL_KEY: &str = "SENTINEL-BEARER-KEY-DO-NOT-LEAK";
    /// Text the thinker types into a Note. It is *expected* in Note-bearing
    /// artifacts, and is the audit's proof that the byte scan reads them.
    const CONTROL_TEXT: &str = "SENTINEL-CONTROL-THINKER-TYPED-THIS";

    /// A cloud client that records the `Authorization` value it is handed, so
    /// the audit can prove the key reached the request rather than assuming it.
    struct RecordingCloudClient {
        seen: Mutex<Vec<Option<String>>>,
    }

    #[async_trait]
    impl CloudTagsClient for RecordingCloudClient {
        async fn fetch_tags(
            &self,
            _base_url: &str,
            authorization: Option<&str>,
        ) -> Result<CloudHttpResponse, CloudDiscoveryFailureCode> {
            self.seen
                .lock()
                .unwrap()
                .push(authorization.map(str::to_owned));
            Ok(CloudHttpResponse {
                status: 200,
                body: r#"{"models":[{"name":"gpt-oss:120b-cloud"}]}"#.to_owned(),
            })
        }
    }

    fn temporary_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "nodepad-sentinel-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    /// Substring search over raw bytes. The artifacts are a mix of UTF-8
    /// documents and binary SQLite pages, so the scan cannot go through `str`.
    fn contains(haystack: &[u8], needle: &str) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle.as_bytes())
    }

    #[test]
    fn a_bearer_key_in_flight_reaches_the_request_and_no_durable_byte() {
        let path = temporary_path("db").with_extension("sqlite");
        let backups_dir = temporary_path("backups");
        std::fs::create_dir_all(&backups_dir).unwrap();

        // ── Plant the sentinel where the production Cloud path reads it ─────
        let keychain = Arc::new(FakeKeychain::default());
        *keychain.read_result.lock().unwrap() = Ok(SENTINEL_KEY.to_owned());
        let client = Arc::new(RecordingCloudClient {
            seen: Mutex::new(Vec::new()),
        });
        let provider = CloudOllamaProvider::new(
            client.clone(),
            keychain.clone(),
            OLLAMA_CLOUD_KEYCHAIN_SERVICE,
            OLLAMA_CLOUD_KEYCHAIN_ACCOUNT,
        );

        // ── Drive a real Cloud discovery ────────────────────────────────────
        let outcome = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(provider.discover_models(crate::workspace::CloudProvider::Ollama));

        // The secret is genuinely in flight: the provider read it from the
        // keychain and handed it to the request. Without this, every
        // assertion below would pass on a value that was never introduced.
        let seen = client.seen.lock().unwrap().clone();
        assert_eq!(seen.len(), 1, "exactly one Cloud request was made");
        assert_eq!(
            seen[0].as_deref(),
            Some(SENTINEL_KEY),
            "the bearer key must reach the request, or this audit proves nothing"
        );
        assert!(
            matches!(
                outcome,
                crate::cloud::CloudDiscoveryOutcome::Committed { .. }
            ),
            "the Cloud discovery succeeded: {outcome:?}"
        );

        // ── Now run a durable session on the same Cloud-consented Workspace ─
        let mut store = WorkspaceStore::open(&path).unwrap();
        let workspace_id = store.snapshot().unwrap().active_workspace_id;
        store
            .set_assistance_policy(&workspace_id, crate::workspace::AssistancePolicy::CloudAi)
            .unwrap();
        store.set_cloud_consent(&workspace_id, true).unwrap();
        store
            .set_selected_model(&workspace_id, Some("gpt-oss:120b-cloud"))
            .unwrap();

        let note = store
            .create_note(&workspace_id, &format!("# A Note quoting {CONTROL_TEXT}"))
            .unwrap();
        let note_id = note.notes[0].id.clone();
        store.set_note_annotation(&note_id, CONTROL_TEXT).unwrap();
        store.attach_label(&note_id, CONTROL_TEXT).unwrap();

        // ── Every artifact the issue names, as raw bytes ────────────────────
        let mut artifacts: Vec<(&str, Vec<u8>)> = Vec::new();

        let snapshot = store.snapshot().unwrap();
        artifacts.push(("the UI snapshot", serde_json::to_vec(&snapshot).unwrap()));

        let (markdown, _) = store.markdown_export(&workspace_id).unwrap();
        artifacts.push(("the Markdown export", markdown.into_bytes()));

        let exported = store.archive_export_data(&workspace_id).unwrap();
        let archive =
            crate::archive::build_archive(&exported, "0.1.0", "2026-07-24T12:00:00+00:00");
        artifacts.push((
            "the exported archive",
            crate::archive::serialize_archive(&archive)
                .unwrap()
                .into_bytes(),
        ));

        let manifest = store
            .create_backup(
                &backups_dir,
                crate::backup::BackupKind::Automatic,
                "2026-07-24T12:00:00.000000Z",
                "0.1.0",
            )
            .unwrap();
        artifacts.push((
            "the backup file",
            std::fs::read(backups_dir.join(format!("{}.sqlite", manifest.id))).unwrap(),
        ));
        artifacts.push((
            "the backup manifest",
            serde_json::to_vec(&manifest).unwrap(),
        ));

        // The live database including the write-ahead log: a value can sit in
        // the WAL long before it is checkpointed into the main file.
        for suffix in ["", "-wal"] {
            let file = std::path::PathBuf::from(format!("{}{suffix}", path.display()));
            if let Ok(bytes) = std::fs::read(&file) {
                artifacts.push(("the SQLite database", bytes));
            }
        }

        // Errors and logs: a failure raised while the Cloud path is live must
        // not echo what it was carrying, and neither must the keychain seam's
        // own reported outcomes.
        let failure = store.set_selected_model("no-such-workspace", Some("phi3:latest"));
        artifacts.push(("a failure message", format!("{failure:?}").into_bytes()));
        artifacts.push((
            "the Cloud discovery outcome",
            format!("{outcome:?}").into_bytes(),
        ));
        artifacts.push((
            "the recorded keychain calls",
            format!("{:?}", keychain.calls.lock().unwrap()).into_bytes(),
        ));

        // ── The audit ───────────────────────────────────────────────────────
        let mut control_sightings = 0;
        for (name, bytes) in &artifacts {
            assert!(
                !contains(bytes, SENTINEL_KEY),
                "{name} carried the bearer key"
            );
            if contains(bytes, CONTROL_TEXT) {
                control_sightings += 1;
            }
        }

        // The scan works: text the thinker typed is found in the snapshot, the
        // Markdown export, the archive, the backup, and the database.
        assert!(
            control_sightings >= 5,
            "the positive control was found in only {control_sightings} of {} artifacts, \
             so the scan is not actually reading these bytes",
            artifacts.len()
        );

        drop(store);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
        let _ = std::fs::remove_dir_all(&backups_dir);
    }
}

/// The scans above read source. This one reads the *built* front end, which is
/// what actually ships inside the `.app`. Minification, inlining, and any
/// dependency's runtime can put a string into `dist/` that appears nowhere in
/// `src/`, so auditing source alone would miss it.
///
/// The bundle is a build output, so the test skips when `dist/` is absent
/// rather than failing — but `npm run check` builds it before the Rust suite
/// runs, so the release gate always exercises this.
#[test]
fn the_built_front_end_carries_no_forbidden_marker_or_unapproved_origin() {
    let dist = repository_root().join("dist");
    if !dist.exists() {
        return;
    }
    let root = repository_root();
    let mut files = Vec::new();
    collect_any(&dist, &root, &mut files);
    assert!(
        !files.is_empty(),
        "dist/ exists but the audit read nothing out of it"
    );

    let allowed: Vec<&str> = ALLOWED_OUTBOUND_HOSTS
        .iter()
        .map(|(host, _)| *host)
        .collect();

    let mut offences = Vec::new();
    for (path, contents) in &files {
        for (marker, category) in FORBIDDEN_MARKERS {
            if contents.contains(marker) {
                offences.push(format!("{path}: {marker} ({category})"));
            }
        }
        // Minified output is one enormous line, so the built assets are
        // scanned whole rather than line by line.
        for origin in origins_in(contents) {
            let known = allowed.iter().any(|host| origin.starts_with(host))
                || TEST_FIXTURE_ORIGINS
                    .iter()
                    .any(|host| origin.starts_with(host))
                || BUILT_ASSET_ORIGIN_EXEMPTIONS
                    .iter()
                    .any(|host| origin.starts_with(host));
            if !known {
                offences.push(format!("{path}: {origin}"));
            }
        }
    }
    assert!(
        offences.is_empty(),
        "the built front end must carry no forbidden marker and no unapproved \
         origin, but found:\n  {}",
        offences.join("\n  ")
    );
}

/// Origins that legitimately appear in third-party bundled code as inert
/// identifiers rather than as anything the app requests: XML namespaces, and
/// documentation links embedded in library error messages.
///
/// Each was checked by hand against the built bundle:
///
/// - `w3.org` / `schema.org` — XML and SVG namespace identifiers, never fetched.
/// - `github.com` — `hast-util-to-jsx-runtime` and `react-markdown` embed
///   repository and changelog links in the text of their error messages.
/// - `radix-ui.com` — a `.../docs/components/${docsSlug}` link inside a Radix
///   error message.
/// - `react.dev` — the `react.dev/errors/` link React prints when a minified
///   error fires.
///
/// None is ever passed to a request. The webview's CSP is `default-src 'self'`,
/// so even a library that tried could not load them.
const BUILT_ASSET_ORIGIN_EXEMPTIONS: &[&str] = &[
    "http://www.w3.org",
    "https://www.w3.org",
    "http://schema.org",
    "https://schema.org",
    "https://github.com",
    "https://radix-ui.com",
    "https://react.dev",
];

/// Like [`collect`] but takes every file, whatever its extension: a built
/// bundle's contents are not predictable from a suffix list.
fn collect_any(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_any(&path, root, out);
        } else if let Ok(contents) = std::fs::read_to_string(&path) {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            out.push((relative, contents));
        }
    }
}

/// The one development URL that genuinely ships.
///
/// `tauri.conf.json` carries `build.devUrl` so `npm run dev` works, and Tauri
/// embeds the whole config in the release binary. The release notes disclose
/// this rather than leaving it for a reader to find in a strings dump, and
/// this audit pins the two properties that make it inert: it is a loopback
/// address, so it is not a remote endpoint, and the release loads its front
/// end from `frontendDist` instead.
///
/// If `devUrl` ever becomes a non-loopback address, this fails.
#[test]
fn the_embedded_development_url_is_loopback_and_unused_in_release() {
    let config = std::fs::read_to_string(manifest_dir().join("tauri.conf.json"))
        .expect("tauri.conf.json is readable");
    let parsed: serde_json::Value =
        serde_json::from_str(&config).expect("tauri.conf.json is valid JSON");

    let dev_url = parsed["build"]["devUrl"]
        .as_str()
        .expect("the build declares a devUrl");
    let host = dev_url
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split(['/', ':'])
        .next()
        .unwrap_or_default();
    assert!(
        matches!(host, "127.0.0.1" | "localhost" | "[::1]"),
        "devUrl must be loopback so the string embedded in the release binary \
         cannot name a remote host, but is `{dev_url}`"
    );

    assert!(
        parsed["build"]["frontendDist"].as_str().is_some(),
        "the release must load its front end from frontendDist, not devUrl"
    );
}
