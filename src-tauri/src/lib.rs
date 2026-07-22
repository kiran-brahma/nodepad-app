mod workspace;

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};
use workspace::{
    StorageOpenFailure, StorageOpenFailureCategory, ThinkingWorkspaceInterface,
    WorkspaceCommandResult, WorkspaceStore,
};

/// Storage may be unavailable at startup; the app stays running so the thinker
/// can retry or quit without the database ever being reset.
struct AppState {
    database_path: PathBuf,
    storage: Mutex<Result<WorkspaceStore, StorageOpenFailure>>,
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

/// Retries the open that failed. It reuses the same path and never recreates it.
#[tauri::command]
fn retry_storage_open(state: State<'_, AppState>) -> WorkspaceCommandResult {
    let mut storage = state.storage.lock().expect("workspace lock poisoned");
    if storage.is_err() {
        *storage = WorkspaceStore::open(&state.database_path);
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

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let storage = match std::fs::create_dir_all(&data_dir) {
                Ok(()) => WorkspaceStore::open(data_dir.join("nodepad.sqlite")),
                Err(error) => Err(StorageOpenFailure::new(
                    StorageOpenFailureCategory::Initialization,
                    format!("Nodepad could not reach its local data folder: {error}"),
                )),
            };
            app.manage(AppState {
                database_path: data_dir.join("nodepad.sqlite"),
                storage: Mutex::new(storage),
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
            retry_storage_open,
            quit_application
        ])
        .run(tauri::generate_context!())
        .expect("error while running Nodepad");
}
