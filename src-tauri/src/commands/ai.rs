use crate::{
    domain::{AiSettingsInput, AiSettingsView},
    providers::ai,
    AppState,
};
use tauri::State;

#[tauri::command]
pub fn get_ai_settings(state: State<'_, AppState>) -> Result<AiSettingsView, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "本地数据库锁定失败".to_string())?;
    Ok(ai::get_settings(&db))
}

#[tauri::command]
pub fn save_ai_settings(
    input: AiSettingsInput,
    state: State<'_, AppState>,
) -> Result<AiSettingsView, String> {
    let mut db = state
        .db
        .lock()
        .map_err(|_| "本地数据库锁定失败".to_string())?;
    ai::save_settings(&mut db, input)
}
