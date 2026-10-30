use crate::{
    domain::{AppStatus, DataStatus, GamePhase},
    AppState,
};
use rusqlite::OptionalExtension;
use tauri::State;

#[tauri::command]
pub fn get_app_status(state: State<'_, AppState>) -> Result<AppStatus, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "本地数据库锁定失败".to_string())?;
    let snapshot = db
        .query_row(
            "SELECT source, patch, updated_at, stale FROM snapshot_metadata WHERE key = 'rankings'",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, bool>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|error| format!("读取榜单状态失败：{error}"))?;
    let augment_snapshot = db
        .query_row(
            "SELECT source, patch, updated_at, stale FROM snapshot_metadata WHERE key = 'augment_grades'",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, bool>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|error| format!("读取海克斯数据状态失败：{error}"))?;

    let data_status = match snapshot {
        Some((source, patch, updated_at, stale)) => DataStatus {
            ready: true,
            source: Some(source),
            patch,
            updated_at,
            stale,
            augment_ready: augment_snapshot.is_some(),
            augment_source: augment_snapshot.as_ref().map(|value| value.0.clone()),
            augment_patch: augment_snapshot.as_ref().and_then(|value| value.1.clone()),
            augment_updated_at: augment_snapshot.as_ref().and_then(|value| value.2.clone()),
            augment_stale: augment_snapshot.as_ref().is_some_and(|value| value.3),
        },
        None => DataStatus {
            ready: false,
            source: None,
            patch: None,
            updated_at: None,
            stale: false,
            augment_ready: augment_snapshot.is_some(),
            augment_source: augment_snapshot.as_ref().map(|value| value.0.clone()),
            augment_patch: augment_snapshot.as_ref().and_then(|value| value.1.clone()),
            augment_updated_at: augment_snapshot.as_ref().and_then(|value| value.2.clone()),
            augment_stale: augment_snapshot.as_ref().is_some_and(|value| value.3),
        },
    };

    Ok(AppStatus {
        version: env!("CARGO_PKG_VERSION").to_string(),
        data_status,
        game_phase: GamePhase::Disconnected,
    })
}
