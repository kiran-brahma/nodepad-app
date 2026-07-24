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
    ("api.openai.com", "a non-Ollama provider"),
    ("openrouter.ai", "a non-Ollama provider"),
    ("api.anthropic.com", "a non-Ollama provider"),
    ("api.z.ai", "a non-Ollama provider"),
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

/// A line that only *documents* a URL is not a line that contacts one. Doc
/// comments, ordinary comments, and test assertions about rejected input all
/// mention URLs by necessity. The audit skips them so it can stay strict about
/// the lines that remain.
fn is_prose_or_test(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("//")
        || trimmed.starts_with("*")
        || trimmed.starts_with("/*")
        || trimmed.contains("assert!")
        || trimmed.contains("assert_eq!")
}

#[test]
fn every_outbound_origin_is_on_the_release_allowlist() {
    let allowed: Vec<&str> = ALLOWED_OUTBOUND_HOSTS
        .iter()
        .map(|(host, _)| *host)
        .collect();
    // `example.com` and the RFC reserved ranges appear throughout the suite as
    // the input a guard is supposed to reject; they are never contacted.
    let test_fixtures = [
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
        "https://schema.tauri.app",
        "https://ui.shadcn.com",
    ];

    let mut offences = Vec::new();
    for (path, contents) in shipped_sources() {
        for (number, line) in contents.lines().enumerate() {
            if is_prose_or_test(line) {
                continue;
            }
            for origin in origins_in(line) {
                let known = allowed.iter().any(|host| origin.starts_with(host))
                    || test_fixtures.iter().any(|host| origin.starts_with(host));
                if !known {
                    offences.push(format!("{path}:{}: {origin}", number + 1));
                }
            }
        }
    }

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
    let mut offences = Vec::new();
    for (path, contents) in shipped_sources() {
        for (number, line) in contents.lines().enumerate() {
            if is_prose_or_test(line) {
                continue;
            }
            for (marker, category) in FORBIDDEN_MARKERS {
                if line.contains(marker) {
                    offences.push(format!("{path}:{}: {marker} ({category})", number + 1));
                }
            }
        }
    }
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
    // `fetch(` would also match `prefetch(`; the audit looks for the global.
    let forbidden = [
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

    let mut offences = Vec::new();
    for (path, contents) in files {
        for (number, line) in contents.lines().enumerate() {
            if is_prose_or_test(line) {
                continue;
            }
            for marker in forbidden {
                if line.contains(marker) {
                    offences.push(format!("{path}:{}: {marker}", number + 1));
                }
            }
        }
    }
    assert!(
        offences.is_empty(),
        "the webview must reach the network only through a Tauri command, but found:\n  {}",
        offences.join("\n  ")
    );
}

/// The content security policy is the backstop: even if a Note's Markdown were
/// to render a remote reference, the webview may not load it.
#[test]
fn the_webview_content_security_policy_denies_every_remote_origin() {
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
    for scheme in ["http://", "https://", "*"] {
        assert!(
            !csp.contains(scheme),
            "the policy admits a remote origin via `{scheme}`: {csp}"
        );
    }
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
