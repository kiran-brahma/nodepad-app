mod cloud;
mod ollama;
mod secrets;
mod thinking_graph;
mod workspace;

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, State};
use cloud::{CloudOllamaProvider, CloudTagsClient, HttpCloudTagsClient};
use ollama::{DiscoveryOutcome, HttpTagsClient, OllamaProvider};
use secrets::{
    KeychainAdapter, KeychainFailure, KeychainOutcome, OLLAMA_CLOUD_KEYCHAIN_ACCOUNT,
    OLLAMA_CLOUD_KEYCHAIN_SERVICE, SecurityCliKeychain,
};
use workspace::{
    unavailable_search_outcome, StorageOpenFailure, StorageOpenFailureCategory, ThinkingWorkspaceInterface,
    WorkspaceCommandResult, WorkspaceSearchOutcome, WorkspaceStore,
};

/// Storage may be unavailable at startup; the app stays running so the thinker
/// can retry or quit without the database ever being reset.
struct AppState {
    storage: Mutex<Result<WorkspaceStore, StorageOpenFailure>>,
    provider: OllamaProvider,
    cloud: CloudOllamaProvider,
    keychain: Arc<dyn KeychainAdapter>,
}

impl AppState {
    fn dispatch(
        &self,
        intent: impl FnOnce(&mut WorkspaceStore) -> WorkspaceCommandResult,
    ) -> WorkspaceCommandResult {
        match self.storage.lock().expect("workspace lock poisoned").as_mut() {
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
        match self.storage.lock().expect("workspace lock poisoned").as_ref() {
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
fn attach_label(note_id: String, name: String, state: State<'_, AppState>) -> WorkspaceCommandResult {
    state.dispatch(|store| store.attach_label_outcome(&note_id, &name))
}

#[tauri::command]
fn detach_label(note_id: String, label_id: String, state: State<'_, AppState>) -> WorkspaceCommandResult {
    state.dispatch(|store| store.detach_label_outcome(&note_id, &label_id))
}

#[tauri::command]
fn rename_label(label_id: String, name: String, state: State<'_, AppState>) -> WorkspaceCommandResult {
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
fn search_notes(workspace_id: String, query: String, state: State<'_, AppState>) -> WorkspaceSearchOutcome {
    state.dispatch_search(|store| store.search_outcome(&workspace_id, &query))
}

/// Undoes the most recent reversible change in this Workspace by committing a
/// compensating transaction. History lives only in this process.
#[tauri::command]
fn undo_last_change(workspace_id: String, state: State<'_, AppState>) -> WorkspaceCommandResult {
    state.dispatch(|store| store.undo_outcome(&workspace_id))
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
    Ok(secret_outcome(
        state
            .keychain
            .delete(OLLAMA_CLOUD_KEYCHAIN_SERVICE, OLLAMA_CLOUD_KEYCHAIN_ACCOUNT),
    ))
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
                Arc::new(HttpCloudTagsClient::new(http_client));
            let keychain: Arc<dyn KeychainAdapter> = Arc::new(SecurityCliKeychain::new());
            let cloud = CloudOllamaProvider::new(
                cloud_client,
                keychain.clone(),
                OLLAMA_CLOUD_KEYCHAIN_SERVICE,
                OLLAMA_CLOUD_KEYCHAIN_ACCOUNT,
            );
            app.manage(AppState {
                storage: Mutex::new(storage),
                provider,
                cloud,
                keychain,
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
            set_assistance_policy,
            set_cloud_consent,
            select_model,
            discover_local_models,
            discover_cloud_models,
            cloud_api_key_present,
            set_cloud_api_key,
            delete_cloud_api_key,
            retry_storage_open,
            quit_application
        ])
        .run(tauri::generate_context!())
        .expect("error while running Nodepad");
}
