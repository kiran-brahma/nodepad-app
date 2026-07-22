mod workspace;

use std::sync::Mutex;
use tauri::{Manager, State};
use workspace::{ThinkingWorkspaceInterface, WorkspaceCommandResult, WorkspaceStore};

struct AppState(Mutex<WorkspaceStore>);

#[tauri::command]
fn get_workspace_snapshot(state: State<'_, AppState>) -> WorkspaceCommandResult {
    state
        .0
        .lock()
        .expect("workspace lock poisoned")
        .snapshot_outcome()
}

#[tauri::command]
fn create_workspace(name: String, state: State<'_, AppState>) -> WorkspaceCommandResult {
    state
        .0
        .lock()
        .expect("workspace lock poisoned")
        .create_workspace_outcome(&name)
}

#[tauri::command]
fn create_note(
    workspace_id: String,
    markdown: String,
    state: State<'_, AppState>,
) -> WorkspaceCommandResult {
    state
        .0
        .lock()
        .expect("workspace lock poisoned")
        .create_note_outcome(&workspace_id, &markdown)
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let store = WorkspaceStore::open(data_dir.join("nodepad.sqlite"))
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            app.manage(AppState(Mutex::new(store)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_workspace_snapshot,
            create_workspace,
            create_note
        ])
        .run(tauri::generate_context!())
        .expect("error while running Nodepad");
}
