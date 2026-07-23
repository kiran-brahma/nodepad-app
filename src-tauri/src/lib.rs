mod archive;
mod cloud;
mod enrichment;
mod markdown_export;
mod ollama;
mod secrets;
mod synthesis;
mod thinking_graph;
mod url_metadata;
mod workspace;

use cloud::{CloudOllamaProvider, CloudTagsClient, HttpCloudTagsClient, OLLAMA_CLOUD_BASE_URL};
use enrichment::{
    EnrichmentClient, EnrichmentFailureCode, EnrichmentOutcome, EnrichmentRequest,
    HttpEnrichmentClient, ParsedEnrichmentResult, RequestToken,
};
use ollama::{
    DiscoveryOutcome, HttpTagsClient, OllamaProvider,
    OLLAMA_LOCAL_BASE_URL_PUBLIC as OLLAMA_LOCAL_BASE_URL,
};
use secrets::{
    KeychainAdapter, KeychainFailure, KeychainOutcome, SecurityCliKeychain,
    OLLAMA_CLOUD_KEYCHAIN_ACCOUNT, OLLAMA_CLOUD_KEYCHAIN_SERVICE,
};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use url_metadata::{HttpUrlMetadataClient, UrlMetadataClient};
use workspace::{
    unavailable_search_outcome, AssistancePolicy, StorageOpenFailure, StorageOpenFailureCategory,
    ThinkingWorkspaceInterface, WorkspaceCommandResult, WorkspaceFailureCode,
    WorkspaceSearchOutcome, WorkspaceSnapshot, WorkspaceStore,
};

/// Storage may be unavailable at startup; the app stays running so the thinker
/// can retry or quit without the database ever being reset.
struct AppState {
    storage: Mutex<Result<WorkspaceStore, StorageOpenFailure>>,
    provider: OllamaProvider,
    cloud: CloudOllamaProvider,
    keychain: Arc<dyn KeychainAdapter>,
    /// The HTTP client behind local `/api/chat`. The same builder is used
    /// for local and cloud chat calls; only the endpoint URL and the
    /// optional bearer key differ.
    local_enrichment: HttpEnrichmentClient,
    cloud_enrichment: HttpEnrichmentClient,
    url_metadata: HttpUrlMetadataClient,
}

impl AppState {
    fn dispatch(
        &self,
        intent: impl FnOnce(&mut WorkspaceStore) -> WorkspaceCommandResult,
    ) -> WorkspaceCommandResult {
        match self
            .storage
            .lock()
            .expect("workspace lock poisoned")
            .as_mut()
        {
            Ok(store) => intent(store),
            Err(failure) => WorkspaceCommandResult::Unavailable {
                failure: failure.clone(),
            },
        }
    }

    fn dispatch_search(
        &self,
        intent: impl FnOnce(&WorkspaceStore) -> WorkspaceSearchOutcome,
    ) -> WorkspaceSearchOutcome {
        match self
            .storage
            .lock()
            .expect("workspace lock poisoned")
            .as_ref()
        {
            Ok(store) => intent(store),
            Err(failure) => unavailable_search_outcome(failure),
        }
    }
}

#[tauri::command]
fn get_workspace_snapshot(state: State<'_, AppState>) -> WorkspaceCommandResult {
    state.dispatch(|store| store.snapshot_outcome())
}

#[tauri::command]
fn create_workspace(name: String, state: State<'_, AppState>) -> WorkspaceCommandResult {
    state.dispatch(|store| store.create_workspace_outcome(&name))
}

#[tauri::command]
fn select_workspace(workspace_id: String, state: State<'_, AppState>) -> WorkspaceCommandResult {
    state.dispatch(|store| store.select_workspace_outcome(&workspace_id))
}

#[tauri::command]
fn rename_workspace(
    workspace_id: String,
    name: String,
    state: State<'_, AppState>,
) -> WorkspaceCommandResult {
    state.dispatch(|store| store.rename_workspace_outcome(&workspace_id, &name))
}

#[tauri::command]
fn delete_workspace(workspace_id: String, state: State<'_, AppState>) -> WorkspaceCommandResult {
    state.dispatch(|store| store.delete_workspace_outcome(&workspace_id))
}

#[tauri::command]
fn create_note(
    workspace_id: String,
    markdown: String,
    state: State<'_, AppState>,
) -> WorkspaceCommandResult {
    state.dispatch(|store| store.create_note_outcome(&workspace_id, &markdown))
}

#[tauri::command]
fn edit_note_text(
    note_id: String,
    markdown: String,
    state: State<'_, AppState>,
) -> WorkspaceCommandResult {
    state.dispatch(|store| store.edit_note_text_outcome(&note_id, &markdown))
}

#[tauri::command]
fn set_note_type(
    note_id: String,
    note_type: String,
    state: State<'_, AppState>,
) -> WorkspaceCommandResult {
    state.dispatch(|store| store.set_note_type_outcome(&note_id, &note_type))
}

#[tauri::command]
fn set_note_annotation(
    note_id: String,
    annotation: String,
    state: State<'_, AppState>,
) -> WorkspaceCommandResult {
    state.dispatch(|store| store.set_note_annotation_outcome(&note_id, &annotation))
}

#[tauri::command]
fn set_note_pinned(
    note_id: String,
    pinned: bool,
    state: State<'_, AppState>,
) -> WorkspaceCommandResult {
    state.dispatch(|store| store.set_note_pinned_outcome(&note_id, pinned))
}

#[tauri::command]
fn delete_note(note_id: String, state: State<'_, AppState>) -> WorkspaceCommandResult {
    state.dispatch(|store| store.delete_note_outcome(&note_id))
}

/// Moves a Note into another Thinking Workspace, keeping its identity and
/// authored fields, remapping its Labels, and removing every Relationship.
#[tauri::command]
fn move_note(
    note_id: String,
    target_workspace_id: String,
    state: State<'_, AppState>,
) -> WorkspaceCommandResult {
    state.dispatch(|store| store.move_note_outcome(&note_id, &target_workspace_id))
}

/// Copies a Note into another Thinking Workspace under a fresh identity, with
/// no Relationship.
#[tauri::command]
fn copy_note(
    note_id: String,
    target_workspace_id: String,
    state: State<'_, AppState>,
) -> WorkspaceCommandResult {
    state.dispatch(|store| store.copy_note_outcome(&note_id, &target_workspace_id))
}

#[tauri::command]
fn attach_label(
    note_id: String,
    name: String,
    state: State<'_, AppState>,
) -> WorkspaceCommandResult {
    state.dispatch(|store| store.attach_label_outcome(&note_id, &name))
}

#[tauri::command]
fn detach_label(
    note_id: String,
    label_id: String,
    state: State<'_, AppState>,
) -> WorkspaceCommandResult {
    state.dispatch(|store| store.detach_label_outcome(&note_id, &label_id))
}

#[tauri::command]
fn rename_label(
    label_id: String,
    name: String,
    state: State<'_, AppState>,
) -> WorkspaceCommandResult {
    state.dispatch(|store| store.rename_label_outcome(&label_id, &name))
}

#[tauri::command]
fn remove_label(label_id: String, state: State<'_, AppState>) -> WorkspaceCommandResult {
    state.dispatch(|store| store.remove_label_outcome(&label_id))
}

/// Records one symmetric, untyped Relationship with manual provenance. Asking
/// for one that already exists is not an error and adds no second row.
#[tauri::command]
fn relate_notes(
    note_id: String,
    other_note_id: String,
    state: State<'_, AppState>,
) -> WorkspaceCommandResult {
    state.dispatch(|store| store.relate_notes_outcome(&note_id, &other_note_id))
}

#[tauri::command]
fn unrelate_notes(
    note_id: String,
    other_note_id: String,
    state: State<'_, AppState>,
) -> WorkspaceCommandResult {
    state.dispatch(|store| store.unrelate_notes_outcome(&note_id, &other_note_id))
}

#[tauri::command]
fn search_notes(
    workspace_id: String,
    query: String,
    state: State<'_, AppState>,
) -> WorkspaceSearchOutcome {
    state.dispatch_search(|store| store.search_outcome(&workspace_id, &query))
}

/// Undoes the most recent reversible change in this Workspace by committing a
/// compensating transaction. History lives only in this process.
#[tauri::command]
fn undo_last_change(workspace_id: String, state: State<'_, AppState>) -> WorkspaceCommandResult {
    state.dispatch(|store| store.undo_outcome(&workspace_id))
}

#[derive(serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum MarkdownExportOutcome {
    Exported { filename: String },
    Cancelled,
    Failed { message: String },
}

#[tauri::command]
fn export_workspace(
    workspace_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> MarkdownExportOutcome {
    let document = match state
        .storage
        .lock()
        .expect("workspace lock poisoned")
        .as_ref()
    {
        Ok(store) => store.markdown_export(&workspace_id),
        Err(failure) => {
            return MarkdownExportOutcome::Failed {
                message: failure.message.clone(),
            }
        }
    };
    let (markdown, filename) = match document {
        Ok(document) => document,
        Err(error) => {
            return MarkdownExportOutcome::Failed {
                message: error.failure().message,
            }
        }
    };
    let destination = app
        .dialog()
        .file()
        .add_filter("Markdown", &["md"])
        .set_file_name(&filename)
        .blocking_save_file();
    let Some(destination) = destination else {
        return MarkdownExportOutcome::Cancelled;
    };
    let Some(destination) = destination.as_path() else {
        return MarkdownExportOutcome::Failed {
            message: "Nodepad could not use the selected export location.".to_owned(),
        };
    };
    match markdown_export::write_atomically(destination, &markdown) {
        Ok(()) => MarkdownExportOutcome::Exported { filename },
        Err(error) => MarkdownExportOutcome::Failed {
            message: format!("Nodepad could not export this Thinking Workspace: {error}"),
        },
    }
}

#[derive(serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum ArchiveExportOutcome {
    Exported { filename: String },
    Cancelled,
    Failed { message: String },
}

#[derive(serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum ArchiveImportOutcome {
    Imported { snapshot: WorkspaceSnapshot },
    Cancelled,
    Failed { message: String },
}

/// Exports one Thinking Workspace as a versioned Nodepad archive. The native
/// save dialog chooses the destination; a cancel is a successful no-op. The
/// durable bytes carry no secrets and no transient state.
#[tauri::command]
fn export_workspace_archive(
    workspace_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> ArchiveExportOutcome {
    let data = match state
        .storage
        .lock()
        .expect("workspace lock poisoned")
        .as_ref()
    {
        Ok(store) => store.archive_export_data(&workspace_id),
        Err(failure) => {
            return ArchiveExportOutcome::Failed {
                message: failure.message.clone(),
            }
        }
    };
    let data = match data {
        Ok(data) => data,
        Err(error) => {
            return ArchiveExportOutcome::Failed {
                message: error.failure().message,
            }
        }
    };
    let archive = archive::build_archive(&data, env!("CARGO_PKG_VERSION"), &now_rfc3339());
    let filename = archive::default_filename(&data.workspace.name);
    let json = match archive::serialize_archive(&archive) {
        Ok(json) => json,
        Err(error) => {
            return ArchiveExportOutcome::Failed {
                message: error.message(),
            }
        }
    };
    let destination = app
        .dialog()
        .file()
        .add_filter("Nodepad archive", &["json"])
        .set_file_name(&filename)
        .blocking_save_file();
    let Some(destination) = destination else {
        return ArchiveExportOutcome::Cancelled;
    };
    let Some(destination) = destination.as_path() else {
        return ArchiveExportOutcome::Failed {
            message: "Nodepad could not use the selected archive location.".to_owned(),
        };
    };
    match markdown_export::write_atomically(destination, &json) {
        Ok(()) => ArchiveExportOutcome::Exported { filename },
        Err(error) => ArchiveExportOutcome::Failed {
            message: format!("Nodepad could not write the archive: {error}"),
        },
    }
}

/// Imports one validated V0 archive as a fresh Thinking Workspace. Validation
/// completes before any durable row is touched; a malformed archive fails
/// closed. A cancel of the open dialog is a successful no-op.
#[tauri::command]
fn import_workspace_archive(app: AppHandle, state: State<'_, AppState>) -> ArchiveImportOutcome {
    let Some(source) = app
        .dialog()
        .file()
        .add_filter("Nodepad archive", &["json"])
        .blocking_pick_file()
    else {
        return ArchiveImportOutcome::Cancelled;
    };
    let Some(source) = source.as_path() else {
        return ArchiveImportOutcome::Failed {
            message: "Nodepad could not use the selected archive.".to_owned(),
        };
    };
    let bytes = match std::fs::read(source) {
        Ok(bytes) => bytes,
        Err(error) => {
            return ArchiveImportOutcome::Failed {
                message: format!("Nodepad could not read the archive: {error}"),
            }
        }
    };
    let archive = match archive::parse_and_validate(&bytes) {
        Ok(archive) => archive,
        Err(error) => {
            return ArchiveImportOutcome::Failed {
                message: error.message(),
            }
        }
    };
    let mut storage = state.storage.lock().expect("workspace lock poisoned");
    match storage.as_mut() {
        Ok(store) => match store.import_archive(&archive) {
            Ok(snapshot) => ArchiveImportOutcome::Imported { snapshot },
            Err(error) => ArchiveImportOutcome::Failed {
                message: error.failure().message,
            },
        },
        Err(failure) => ArchiveImportOutcome::Failed {
            message: failure.message.clone(),
        },
    }
}

/// Changes the Assistance Policy of the active Thinking Workspace. Switching
/// to Manual stops future provider calls and invalidates any discovery result
/// that arrives afterwards.
#[tauri::command]
fn set_assistance_policy(
    workspace_id: String,
    policy: String,
    state: State<'_, AppState>,
) -> WorkspaceCommandResult {
    state.dispatch(|store| store.set_assistance_policy_outcome(&workspace_id, &policy))
}

/// Records the opaque model identifier chosen for the active Thinking Workspace.
/// Passing `null` clears the selection.
#[tauri::command]
fn select_model(
    workspace_id: String,
    model_id: Option<String>,
    state: State<'_, AppState>,
) -> WorkspaceCommandResult {
    state.dispatch(|store| store.set_selected_model_outcome(&workspace_id, model_id.as_deref()))
}

/// Discovers models from the fixed local Ollama host. The result is not stored:
/// the UI compares it against the Workspace's selected model and decides what
/// to display.
#[tauri::command]
async fn discover_local_models(state: State<'_, AppState>) -> Result<DiscoveryOutcome, String> {
    Ok(state.provider.discover_models().await)
}

/// Records or revokes the Workspace's affirmative consent to use Ollama Cloud.
/// The bearer key is never stored in the database; this row only names the
/// moment consent was given.
#[tauri::command]
fn set_cloud_consent(
    workspace_id: String,
    accept: bool,
    state: State<'_, AppState>,
) -> WorkspaceCommandResult {
    state.dispatch(|store| store.set_cloud_consent_outcome(&workspace_id, accept))
}

/// Saves the bearer key to the macOS keychain. The key never leaves the
/// keychain after this call; only its presence is read back.
#[tauri::command]
fn set_cloud_api_key(
    api_key: String,
    state: State<'_, AppState>,
) -> Result<CloudSecretOutcome, String> {
    Ok(secret_outcome(state.keychain.write(
        OLLAMA_CLOUD_KEYCHAIN_SERVICE,
        OLLAMA_CLOUD_KEYCHAIN_ACCOUNT,
        &api_key,
    )))
}

/// Removes the bearer key from the macOS keychain. Affected Workspaces
/// surface the absence through the typed discovery failure the next time
/// they try to discover cloud models.
#[tauri::command]
fn delete_cloud_api_key(state: State<'_, AppState>) -> Result<CloudSecretOutcome, String> {
    Ok(secret_outcome(state.keychain.delete(
        OLLAMA_CLOUD_KEYCHAIN_SERVICE,
        OLLAMA_CLOUD_KEYCHAIN_ACCOUNT,
    )))
}

/// Whether a key is currently in the keychain. The key itself is never
/// returned over this seam; only a presence flag.
#[tauri::command]
fn cloud_api_key_present(state: State<'_, AppState>) -> bool {
    state.cloud.has_key()
}

/// Discovers models from the fixed Ollama Cloud host. The call requires both
/// a key in the keychain and consent on the Workspace; the absence of either
/// becomes a typed failure rather than a request the host sees.
#[tauri::command]
async fn discover_cloud_models(
    workspace_id: String,
    state: State<'_, AppState>,
) -> Result<DiscoveryOutcome, String> {
    let consented = state.dispatch(|store| store.snapshot_outcome());
    let workspace = match &consented {
        WorkspaceCommandResult::Committed { snapshot } => snapshot
            .workspaces()
            .iter()
            .find(|workspace| workspace.id() == workspace_id)
            .cloned(),
        _ => None,
    };
    if let Some(workspace) = workspace {
        if workspace.cloud_consent_at().is_none() {
            return Ok(DiscoveryOutcome::Failed {
                failure: ollama::DiscoveryFailure {
                    code: ollama::DiscoveryFailureCode::Unauthenticated,
                    message: "Accept the Cloud AI disclosure to use Ollama Cloud.".into(),
                },
            });
        }
    } else {
        return Ok(DiscoveryOutcome::Failed {
            failure: ollama::DiscoveryFailure {
                code: ollama::DiscoveryFailureCode::Unavailable,
                message: "The active Thinking Workspace could not be found.".into(),
            },
        });
    }
    let outcome = state.cloud.discover_models().await;
    Ok(DiscoveryOutcome::from(outcome))
}

/// The shape returned to the UI: the parsed result (or a typed failure)
/// and, when the result was applied, the new committed snapshot.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum EnrichmentCommandOutcome {
    /// The result was parsed and applied. The snapshot is the new state.
    Applied {
        result: ParsedEnrichmentResult,
        snapshot: WorkspaceSnapshot,
    },
    /// The result was parsed but the gate rejected it (manual provenance,
    /// stale revision, or the policy reverted to Manual mid-flight). The
    /// Note is unchanged.
    Rejected {
        result: ParsedEnrichmentResult,
        snapshot: WorkspaceSnapshot,
        reason: String,
    },
    /// The provider returned something we could not apply. The Note is
    /// unchanged and the UI renders a typed retry state.
    ProviderFailed {
        code: EnrichmentFailureCode,
        message: String,
        snapshot: WorkspaceSnapshot,
    },
    /// The provider returned a body that did not match the structured
    /// contract. The Note is unchanged.
    InvalidSchema {
        reason: String,
        snapshot: WorkspaceSnapshot,
    },
    /// The Workspace is not in a policy that permits AI assistance, the
    /// selected model is missing, or the Note disappeared. The Note is
    /// unchanged.
    Unavailable {
        reason: String,
        snapshot: WorkspaceSnapshot,
    },
}

#[tauri::command]
async fn enrich_note(
    workspace_id: String,
    note_id: String,
    force: bool,
    state: State<'_, AppState>,
) -> Result<EnrichmentCommandOutcome, String> {
    // Step 1: build the request token from the current committed state.
    let (token, request, snapshot) = {
        let storage = state.storage.lock().expect("workspace lock poisoned");
        let store = match storage.as_ref() {
            Ok(store) => store,
            Err(failure) => {
                return Ok(EnrichmentCommandOutcome::Unavailable {
                    reason: failure.message.clone(),
                    snapshot: WorkspaceSnapshot::default_unavailable(),
                });
            }
        };
        let snapshot = match store.snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return Ok(EnrichmentCommandOutcome::Unavailable {
                    reason: error.to_string(),
                    snapshot: WorkspaceSnapshot::default_unavailable(),
                });
            }
        };
        let workspace = match snapshot
            .workspaces
            .iter()
            .find(|candidate| candidate.id == workspace_id)
        {
            Some(workspace) => workspace.clone(),
            None => {
                return Ok(EnrichmentCommandOutcome::Unavailable {
                    reason: "The Thinking Workspace no longer exists.".to_owned(),
                    snapshot,
                });
            }
        };
        let policy = workspace.assistance_policy().as_str().to_owned();
        if !force && matches!(workspace.assistance_policy(), AssistancePolicy::Manual) {
            return Ok(EnrichmentCommandOutcome::Unavailable {
                reason: "AI assistance is not enabled for this Thinking Workspace.".to_owned(),
                snapshot,
            });
        }
        let model = match workspace.selected_model().map(str::to_owned) {
            Some(model) => model,
            None => {
                return Ok(EnrichmentCommandOutcome::Unavailable {
                    reason: "Pick a model in AI Assistance before enriching.".to_owned(),
                    snapshot,
                });
            }
        };
        let note = match snapshot
            .notes
            .iter()
            .find(|candidate| candidate.id() == note_id)
        {
            Some(note) => note.clone(),
            None => {
                return Ok(EnrichmentCommandOutcome::Unavailable {
                    reason: "The Note no longer exists.".to_owned(),
                    snapshot,
                });
            }
        };
        if note.workspace_id() != workspace_id {
            return Ok(EnrichmentCommandOutcome::Unavailable {
                reason: "The Note belongs to a different Thinking Workspace.".to_owned(),
                snapshot,
            });
        }
        let (endpoint, _api_key) = match workspace.assistance_policy() {
            AssistancePolicy::LocalAi => (OLLAMA_LOCAL_BASE_URL.to_owned(), None),
            AssistancePolicy::CloudAi => {
                if workspace.cloud_consent_at().is_none() {
                    return Ok(EnrichmentCommandOutcome::Unavailable {
                        reason: "Accept the Cloud AI disclosure first.".to_owned(),
                        snapshot,
                    });
                }
                match state
                    .keychain
                    .read(OLLAMA_CLOUD_KEYCHAIN_SERVICE, OLLAMA_CLOUD_KEYCHAIN_ACCOUNT)
                {
                    KeychainOutcome::Ok(value) => (OLLAMA_CLOUD_BASE_URL.to_owned(), Some(value)),
                    KeychainOutcome::Failed { failure } => {
                        return Ok(EnrichmentCommandOutcome::Unavailable {
                            reason: failure.message,
                            snapshot,
                        });
                    }
                }
            }
            AssistancePolicy::Manual => {
                return Ok(EnrichmentCommandOutcome::Unavailable {
                    reason: "AI assistance is not enabled for this Thinking Workspace.".to_owned(),
                    snapshot,
                });
            }
        };
        let token = RequestToken {
            workspace_id: workspace_id.clone(),
            note_id: note_id.clone(),
            revision: note.enrichment_revision(),
            policy: policy.clone(),
            endpoint: endpoint.clone(),
            model: model.clone(),
        };
        // The candidate set is read from the same snapshot; the same
        // Workspace's Notes only, with the active Note excluded.
        let candidate_notes: Vec<&workspace::Note> = snapshot
            .notes
            .iter()
            .filter(|candidate| candidate.workspace_id() == workspace_id)
            .collect();
        let candidates = enrichment::select_candidates(&candidate_notes, &note_id);
        let existing_labels: Vec<String> = note
            .labels()
            .iter()
            .map(|label| label.name().to_owned())
            .collect();
        let request = EnrichmentRequest {
            token: token.clone(),
            target_text: note.markdown().to_owned(),
            target_note_id: note.id().to_owned(),
            candidates,
            existing_labels,
            url_metadata: None,
        };
        (token, request, snapshot)
    };
    // URL retrieval is an optional, bounded input. It neither owns Note
    // persistence nor blocks ordinary AI organization when it fails.
    let request = EnrichmentRequest {
        url_metadata: state
            .url_metadata
            .retrieve_from_note(&request.target_text)
            .await,
        ..request
    };
    // Step 2: call the right HTTP client. The bearer key is dropped from
    // this scope the moment the response is in hand.
    let user_message = enrichment::build_user_message(&request);
    let format = enrichment::response_schema();
    let body = match request.token.endpoint.as_str() {
        OLLAMA_LOCAL_BASE_URL => {
            state
                .local_enrichment
                .chat(
                    &request.token.endpoint,
                    &request.token.model,
                    enrichment::SYSTEM_PROMPT,
                    &user_message,
                    &format,
                )
                .await
        }
        OLLAMA_CLOUD_BASE_URL => {
            let key = state
                .keychain
                .read(OLLAMA_CLOUD_KEYCHAIN_SERVICE, OLLAMA_CLOUD_KEYCHAIN_ACCOUNT);
            let key = match key {
                KeychainOutcome::Ok(value) => value,
                KeychainOutcome::Failed { failure } => {
                    return Ok(EnrichmentCommandOutcome::ProviderFailed {
                        code: EnrichmentFailureCode::Unauthenticated,
                        message: failure.message,
                        snapshot,
                    });
                }
            };
            let outcome = state
                .cloud_enrichment
                .chat(
                    &request.token.endpoint,
                    &request.token.model,
                    enrichment::SYSTEM_PROMPT,
                    &user_message,
                    &format,
                )
                .await;
            drop(key);
            outcome
        }
        _ => Err(EnrichmentFailureCode::Unavailable),
    };
    let candidate_ids: Vec<String> = request
        .candidates
        .iter()
        .map(|candidate| candidate.id.clone())
        .collect();
    let outcome = match body {
        Ok(body) => enrichment::parse_enrichment_response(token.clone(), &body, &candidate_ids),
        Err(code) => EnrichmentOutcome::ProviderFailed {
            token: token.clone(),
            code,
            message: "Provider call failed.".to_owned(),
        },
    };
    // Step 3: handle the result. Parsed results are committed; the rest
    // surface as typed failures.
    match outcome {
        EnrichmentOutcome::Parsed {
            token: result_token,
            parsed,
        } => {
            let snapshot = {
                let mut storage = state.storage.lock().expect("workspace lock poisoned");
                match storage.as_mut() {
                    Ok(store) => store.apply_enrichment_outcome(
                        &workspace_id,
                        &note_id,
                        &parsed,
                        &result_token,
                        force,
                    ),
                    Err(failure) => {
                        return Ok(EnrichmentCommandOutcome::Unavailable {
                            reason: failure.message.clone(),
                            snapshot: WorkspaceSnapshot::default_unavailable(),
                        });
                    }
                }
            };
            match snapshot {
                WorkspaceCommandResult::Committed { snapshot } => {
                    Ok(EnrichmentCommandOutcome::Applied {
                        result: parsed,
                        snapshot,
                    })
                }
                WorkspaceCommandResult::Failed { failure }
                    if matches!(failure.code, WorkspaceFailureCode::Stale) =>
                {
                    // A stale response leaves the Note unchanged. The UI
                    // renders the typed retry state on the same view.
                    let current_snapshot = match state
                        .storage
                        .lock()
                        .expect("workspace lock poisoned")
                        .as_ref()
                    {
                        Ok(store) => store
                            .snapshot()
                            .unwrap_or_else(|_| WorkspaceSnapshot::default_unavailable()),
                        Err(_) => WorkspaceSnapshot::default_unavailable(),
                    };
                    Ok(EnrichmentCommandOutcome::Rejected {
                        result: parsed,
                        snapshot: current_snapshot,
                        reason: "The Note was edited during inference. Try again.".to_owned(),
                    })
                }
                WorkspaceCommandResult::Failed { failure } => {
                    Ok(EnrichmentCommandOutcome::Unavailable {
                        reason: failure.message,
                        snapshot: WorkspaceSnapshot::default_unavailable(),
                    })
                }
                WorkspaceCommandResult::Unavailable { failure } => {
                    Ok(EnrichmentCommandOutcome::Unavailable {
                        reason: failure.message,
                        snapshot: WorkspaceSnapshot::default_unavailable(),
                    })
                }
            }
        }
        EnrichmentOutcome::InvalidSchema { reason, .. } => {
            Ok(EnrichmentCommandOutcome::InvalidSchema { reason, snapshot })
        }
        EnrichmentOutcome::ProviderFailed { code, message, .. } => {
            Ok(EnrichmentCommandOutcome::ProviderFailed {
                code,
                message,
                snapshot,
            })
        }
    }
}

/// What one Synthesis attempt reports back. Every variant carries the
/// snapshot the UI should render next, so a refused attempt still leaves
/// the thinker looking at current durable state.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum SynthesisCommandOutcome {
    /// A valid, novel result is now pending. It is not a Note.
    Proposed {
        synthesis: workspace::PendingSynthesis,
        snapshot: WorkspaceSnapshot,
    },
    /// The attempt ran and produced nothing to propose — either the model
    /// returned `found: false`, or the result repeated one the Workspace
    /// has already seen. This is a success: the checkpoint and the cooldown
    /// move, and the UI shows no error.
    NoInsight { snapshot: WorkspaceSnapshot },
    /// The attempt did not run. Every reason is an ordinary state, not a
    /// failure, and a Manual Workspace always lands here.
    Ineligible {
        reason: synthesis::IneligibleReason,
        message: String,
        snapshot: WorkspaceSnapshot,
    },
    /// A source Note changed, was deleted, or left the Workspace while the
    /// request was in flight. Nothing is stored.
    Stale {
        reason: String,
        snapshot: WorkspaceSnapshot,
    },
    InvalidSchema {
        reason: String,
        snapshot: WorkspaceSnapshot,
    },
    ProviderFailed {
        code: EnrichmentFailureCode,
        message: String,
        snapshot: WorkspaceSnapshot,
    },
    Unavailable {
        reason: String,
        snapshot: WorkspaceSnapshot,
    },
}

/// Runs one bounded Synthesis attempt for one Thinking Workspace.
///
/// The shape mirrors `enrich_note`: decide everything against a snapshot
/// taken under the lock, release the lock for the provider call, then
/// re-check and commit. Nothing here mutates a source Note on any path.
#[tauri::command]
async fn propose_synthesis(
    workspace_id: String,
    state: State<'_, AppState>,
) -> Result<SynthesisCommandOutcome, String> {
    let (request, snapshot) = {
        let storage = state.storage.lock().expect("workspace lock poisoned");
        let store = match storage.as_ref() {
            Ok(store) => store,
            Err(failure) => {
                return Ok(SynthesisCommandOutcome::Unavailable {
                    reason: failure.message.clone(),
                    snapshot: WorkspaceSnapshot::default_unavailable(),
                });
            }
        };
        let snapshot = match store.snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return Ok(SynthesisCommandOutcome::Unavailable {
                    reason: error.to_string(),
                    snapshot: WorkspaceSnapshot::default_unavailable(),
                });
            }
        };
        let workspace = match snapshot
            .workspaces()
            .iter()
            .find(|candidate| candidate.id() == workspace_id)
        {
            Some(workspace) => workspace.clone(),
            None => {
                return Ok(SynthesisCommandOutcome::Unavailable {
                    reason: "The Thinking Workspace no longer exists.".to_owned(),
                    snapshot,
                });
            }
        };
        // A Manual Workspace never requests a Synthesis, and neither does
        // one whose Cloud consent or model selection is incomplete.
        let endpoint = match workspace.assistance_policy() {
            AssistancePolicy::Manual => None,
            AssistancePolicy::LocalAi => Some(OLLAMA_LOCAL_BASE_URL.to_owned()),
            AssistancePolicy::CloudAi => workspace
                .cloud_consent_at()
                .map(|_| OLLAMA_CLOUD_BASE_URL.to_owned()),
        };
        let assistance_enabled = endpoint.is_some() && workspace.selected_model().is_some();
        let input = match store.synthesis_eligibility_input(
            &workspace_id,
            assistance_enabled,
            &now_rfc3339(),
        ) {
            Ok(input) => input,
            Err(error) => {
                return Ok(SynthesisCommandOutcome::Unavailable {
                    reason: error.to_string(),
                    snapshot,
                });
            }
        };
        if let synthesis::Eligibility::Ineligible { reason } =
            synthesis::evaluate_eligibility(&input)
        {
            return Ok(SynthesisCommandOutcome::Ineligible {
                reason,
                message: reason.message().to_owned(),
                snapshot,
            });
        }
        // Both were proved present by the eligibility gate above.
        let endpoint = endpoint.unwrap_or_default();
        let model = workspace.selected_model().unwrap_or_default().to_owned();
        let workspace_notes: Vec<&workspace::Note> = snapshot
            .notes
            .iter()
            .filter(|note| note.workspace_id() == workspace_id)
            .collect();
        let candidates = synthesis::select_synthesis_candidates(&workspace_notes);
        if candidates.len() < synthesis::MIN_CANDIDATES {
            let reason = synthesis::IneligibleReason::TooFewOrganizedNotes;
            return Ok(SynthesisCommandOutcome::Ineligible {
                reason,
                message: reason.message().to_owned(),
                snapshot,
            });
        }
        let sources = candidates
            .iter()
            .filter_map(|candidate| {
                workspace_notes
                    .iter()
                    .find(|note| note.id() == candidate.id)
                    .map(|note| synthesis::SourceRevision {
                        note_id: candidate.id.clone(),
                        revision: note.enrichment_revision(),
                    })
            })
            .collect();
        let existing_labels = workspace_label_names(&workspace_notes);
        let previous_syntheses = store
            .previous_synthesis_texts(&workspace_id)
            .unwrap_or_default();
        let request = synthesis::SynthesisRequest {
            token: synthesis::SynthesisRequestToken {
                workspace_id: workspace_id.clone(),
                policy: workspace.assistance_policy(),
                endpoint,
                model,
                sources,
            },
            candidates,
            existing_labels,
            previous_syntheses,
        };
        (request, snapshot)
    };

    let user_message = synthesis::build_user_message(&request);
    let format = synthesis::response_schema();
    let body = match request.token.endpoint.as_str() {
        OLLAMA_LOCAL_BASE_URL => {
            state
                .local_enrichment
                .chat(
                    &request.token.endpoint,
                    &request.token.model,
                    synthesis::SYSTEM_PROMPT,
                    &user_message,
                    &format,
                )
                .await
        }
        OLLAMA_CLOUD_BASE_URL => {
            let key = state
                .keychain
                .read(OLLAMA_CLOUD_KEYCHAIN_SERVICE, OLLAMA_CLOUD_KEYCHAIN_ACCOUNT);
            match key {
                KeychainOutcome::Ok(key) => {
                    let outcome = state
                        .cloud_enrichment
                        .chat(
                            &request.token.endpoint,
                            &request.token.model,
                            synthesis::SYSTEM_PROMPT,
                            &user_message,
                            &format,
                        )
                        .await;
                    drop(key);
                    outcome
                }
                KeychainOutcome::Failed { failure } => {
                    return Ok(SynthesisCommandOutcome::ProviderFailed {
                        code: EnrichmentFailureCode::Unauthenticated,
                        message: failure.message,
                        snapshot,
                    });
                }
            }
        }
        _ => Err(EnrichmentFailureCode::Unavailable),
    };
    let candidate_ids: Vec<String> = request
        .candidates
        .iter()
        .map(|candidate| candidate.id.clone())
        .collect();
    let outcome = match body {
        Ok(body) => {
            synthesis::parse_synthesis_response(request.token.clone(), &body, &candidate_ids)
        }
        Err(code) => synthesis::SynthesisOutcome::ProviderFailed {
            token: request.token.clone(),
            code,
            message: "Provider call failed.".to_owned(),
        },
    };
    Ok(commit_synthesis_outcome(&state, outcome, snapshot))
}

/// Applies one parsed attempt to durable state. A `found: false` result and
/// a semantic repeat both take the same path: record the attempt, store no
/// pending content, and report a quiet no-op.
fn commit_synthesis_outcome(
    state: &State<'_, AppState>,
    outcome: synthesis::SynthesisOutcome,
    snapshot: WorkspaceSnapshot,
) -> SynthesisCommandOutcome {
    let mut storage = state.storage.lock().expect("workspace lock poisoned");
    let store = match storage.as_mut() {
        Ok(store) => store,
        Err(failure) => {
            return SynthesisCommandOutcome::Unavailable {
                reason: failure.message.clone(),
                snapshot: WorkspaceSnapshot::default_unavailable(),
            };
        }
    };
    match outcome {
        synthesis::SynthesisOutcome::NotFound { token } => {
            let _ = store.record_synthesis_attempt(&token.workspace_id);
            SynthesisCommandOutcome::NoInsight {
                snapshot: store.snapshot().unwrap_or(snapshot),
            }
        }
        synthesis::SynthesisOutcome::Proposed { token, result } => {
            let previous = store
                .previous_synthesis_texts(&token.workspace_id)
                .unwrap_or_default();
            if synthesis::is_semantic_repeat(&result.text, &previous) {
                let _ = store.record_synthesis_attempt(&token.workspace_id);
                return SynthesisCommandOutcome::NoInsight {
                    snapshot: store.snapshot().unwrap_or(snapshot),
                };
            }
            let sources: Vec<synthesis::SourceRevision> = result
                .source_note_ids
                .iter()
                .filter_map(|note_id| {
                    token
                        .revision_of(note_id)
                        .map(|revision| synthesis::SourceRevision {
                            note_id: note_id.clone(),
                            revision,
                        })
                })
                .collect();
            if sources.len() != result.source_note_ids.len() {
                let _ = store.record_synthesis_cooldown(&token.workspace_id);
                return SynthesisCommandOutcome::InvalidSchema {
                    reason: "The result named a Note that was not supplied.".to_owned(),
                    snapshot,
                };
            }
            match store.store_pending_synthesis(
                &token.workspace_id,
                &result,
                &sources,
                &token.model,
                token.policy,
            ) {
                Ok(committed) => {
                    let proposed = committed
                        .pending_syntheses()
                        .iter()
                        .filter(|pending| pending.workspace_id == token.workspace_id)
                        .next_back()
                        .cloned();
                    match proposed {
                        Some(synthesis) => SynthesisCommandOutcome::Proposed {
                            synthesis,
                            snapshot: committed,
                        },
                        None => SynthesisCommandOutcome::NoInsight {
                            snapshot: committed,
                        },
                    }
                }
                Err(error) => {
                    // A source moved during inference. Nothing is stored and
                    // no Note is touched; the attempt simply did not land,
                    // and only the cooldown clock moves.
                    let _ = store.record_synthesis_cooldown(&token.workspace_id);
                    let current = store.snapshot().unwrap_or(snapshot);
                    SynthesisCommandOutcome::Stale {
                        reason: error.to_string(),
                        snapshot: current,
                    }
                }
            }
        }
        synthesis::SynthesisOutcome::InvalidSchema { token, reason } => {
            // A failure earns the cooldown but not the checkpoint: a model
            // that reliably answers with nonsense must not be re-asked on
            // every keystroke, and must not consume the thinker's next five
            // organized Notes either.
            let _ = store.record_synthesis_cooldown(&token.workspace_id);
            SynthesisCommandOutcome::InvalidSchema {
                reason,
                snapshot: store.snapshot().unwrap_or(snapshot),
            }
        }
        synthesis::SynthesisOutcome::ProviderFailed {
            token,
            code,
            message,
        } => {
            let _ = store.record_synthesis_cooldown(&token.workspace_id);
            SynthesisCommandOutcome::ProviderFailed {
                code,
                message,
                snapshot: store.snapshot().unwrap_or(snapshot),
            }
        }
    }
}

/// Accepts a pending Synthesis as a fresh thesis Note.
#[tauri::command]
fn accept_synthesis(synthesis_id: String, state: State<'_, AppState>) -> WorkspaceCommandResult {
    state.dispatch(|store| store.accept_synthesis_outcome(&synthesis_id))
}

/// Dismisses a pending Synthesis, keeping only its text for novelty.
#[tauri::command]
fn dismiss_synthesis(synthesis_id: String, state: State<'_, AppState>) -> WorkspaceCommandResult {
    state.dispatch(|store| store.dismiss_synthesis_outcome(&synthesis_id))
}

/// The Workspace's Label vocabulary, deduplicated by display name and
/// bounded, as the `existing_labels` block of Prompt B.
fn workspace_label_names(notes: &[&workspace::Note]) -> Vec<String> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut names = Vec::new();
    for note in notes {
        for label in note.labels() {
            if seen.insert(label.name().to_lowercase()) {
                names.push(label.name().to_owned());
            }
        }
    }
    names
}

/// The current moment, in the same fixed-width UTC shape the durable layer
/// writes, so cooldown comparisons never mix formats.
fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

impl WorkspaceSnapshot {
    /// A placeholder used by the Enrichment Workflow when the durable
    /// store itself is unavailable. The UI treats this as a hard error
    /// and renders the storage recovery affordance rather than a partial
    /// result.
    pub fn default_unavailable() -> Self {
        Self {
            workspaces: vec![],
            notes: vec![],
            relationships: vec![],
            pending_syntheses: vec![],
            active_workspace_id: String::new(),
            undoable_commands: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum CloudSecretOutcome {
    Ok,
    Failed { failure: KeychainFailure },
}

fn secret_outcome(outcome: KeychainOutcome<()>) -> CloudSecretOutcome {
    match outcome {
        KeychainOutcome::Ok(()) => CloudSecretOutcome::Ok,
        KeychainOutcome::Failed { failure } => CloudSecretOutcome::Failed { failure },
    }
}

/// Retries the failed open against the same path, so a folder or permission
/// problem the thinker has since fixed can recover without a restart.
#[tauri::command]
fn retry_storage_open(app: AppHandle, state: State<'_, AppState>) -> WorkspaceCommandResult {
    let mut storage = state.storage.lock().expect("workspace lock poisoned");
    if storage.is_err() {
        *storage = open_storage(&app);
    }
    match storage.as_ref() {
        Ok(store) => store.snapshot_outcome(),
        Err(failure) => WorkspaceCommandResult::Unavailable {
            failure: failure.clone(),
        },
    }
}

#[tauri::command]
fn quit_application(app: AppHandle) {
    app.exit(0);
}

/// Locating and creating the data folder can fail too. Those failures become
/// recovery states rather than aborting startup, so the thinker always reaches
/// a screen that explains the problem instead of a window that never opens.
fn open_storage(app: &AppHandle) -> Result<WorkspaceStore, StorageOpenFailure> {
    let data_dir = app.path().app_data_dir().map_err(|error| {
        StorageOpenFailure::new(
            StorageOpenFailureCategory::Initialization,
            format!("Nodepad could not locate its local data folder: {error}"),
        )
    })?;
    std::fs::create_dir_all(&data_dir).map_err(|error| {
        StorageOpenFailure::new(
            StorageOpenFailureCategory::Initialization,
            format!("Nodepad could not reach its local data folder: {error}"),
        )
    })?;
    WorkspaceStore::open(data_dir.join("nodepad.sqlite"))
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let storage = open_storage(app.handle());
            let http_client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default();
            let provider = OllamaProvider::new(Arc::new(HttpTagsClient::new(http_client.clone())));
            let cloud_client: Arc<dyn CloudTagsClient> =
                Arc::new(HttpCloudTagsClient::new(http_client.clone()));
            let keychain: Arc<dyn KeychainAdapter> = Arc::new(SecurityCliKeychain::new());
            let cloud = CloudOllamaProvider::new(
                cloud_client,
                keychain.clone(),
                OLLAMA_CLOUD_KEYCHAIN_SERVICE,
                OLLAMA_CLOUD_KEYCHAIN_ACCOUNT,
            );
            let local_enrichment = HttpEnrichmentClient::new(http_client.clone(), None);
            let cloud_enrichment = HttpEnrichmentClient::new(http_client, None);
            let url_metadata = HttpUrlMetadataClient::new();
            app.manage(AppState {
                storage: Mutex::new(storage),
                provider,
                cloud,
                keychain,
                local_enrichment,
                cloud_enrichment,
                url_metadata,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_workspace_snapshot,
            create_workspace,
            select_workspace,
            rename_workspace,
            delete_workspace,
            create_note,
            edit_note_text,
            set_note_type,
            set_note_annotation,
            set_note_pinned,
            delete_note,
            move_note,
            copy_note,
            attach_label,
            detach_label,
            rename_label,
            remove_label,
            relate_notes,
            unrelate_notes,
            search_notes,
            undo_last_change,
            export_workspace,
            export_workspace_archive,
            import_workspace_archive,
            set_assistance_policy,
            set_cloud_consent,
            select_model,
            discover_local_models,
            discover_cloud_models,
            cloud_api_key_present,
            set_cloud_api_key,
            delete_cloud_api_key,
            retry_storage_open,
            quit_application,
            enrich_note,
            propose_synthesis,
            accept_synthesis,
            dismiss_synthesis
        ])
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!())
        .expect("error while running Nodepad");
}
