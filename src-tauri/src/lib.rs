mod workspace;

use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};
use workspace::{
    StorageOpenFailure, StorageOpenFailureCategory, ThinkingWorkspaceInterface,
    WorkspaceCommandResult, WorkspaceStore,
};

/// Storage may be unavailable at startup; the app stays running so the thinker
/// can retry or quit without the database ever being reset.
struct AppState {
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

/// Undoes the most recent reversible change in this Workspace by committing a
/// compensating transaction. History lives only in this process.
#[tauri::command]
fn undo_last_change(workspace_id: String, state: State<'_, AppState>) -> WorkspaceCommandResult {
    state.dispatch(|store| store.undo_outcome(&workspace_id))
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
            app.manage(AppState {
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
            edit_note_text,
            set_note_type,
            set_note_annotation,
            set_note_pinned,
            delete_note,
            undo_last_change,
            retry_storage_open,
            quit_application
        ])
        .run(tauri::generate_context!())
        .expect("error while running Nodepad");
}
