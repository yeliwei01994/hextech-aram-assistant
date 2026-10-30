use crate::{db::snapshot, domain::DataStatus, AppState};
use tauri::State;

#[tauri::command]
pub fn import_rankings_json(
    contents: String,
    state: State<'_, AppState>,
) -> Result<DataStatus, String> {
    let mut db = state
        .db
        .lock()
        .map_err(|_| "本地数据库锁定失败".to_string())?;
    snapshot::import_rankings_json(&mut db, &contents)
}

#[tauri::command]
pub fn import_augment_grades_json(
    contents: String,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let mut db = state
        .db
        .lock()
        .map_err(|_| "本地数据库锁定失败".to_string())?;
    snapshot::import_augment_grades_json(&mut db, &contents)
}
