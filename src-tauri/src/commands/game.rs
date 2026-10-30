use crate::{
    domain::{GameDetectionSettings, GameDetectionSettingsInput},
    AppState,
};
use rusqlite::{params, OptionalExtension};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, State};

const DETECTION_SETTING: &str = "game_detection_enabled";

#[tauri::command]
pub fn get_game_detection_settings(
    state: State<'_, AppState>,
) -> Result<GameDetectionSettings, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "本地数据库锁定失败".to_string())?;
    let enabled = db
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [DETECTION_SETTING],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map(|value| value.as_deref() == Some("true"))
        .unwrap_or(false);
    Ok(GameDetectionSettings { enabled })
}

#[tauri::command]
pub fn save_game_detection_settings(
    input: GameDetectionSettingsInput,
    state: State<'_, AppState>,
) -> Result<GameDetectionSettings, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "本地数据库锁定失败".to_string())?;
    db.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![DETECTION_SETTING, input.enabled.to_string()],
    )
    .map_err(|error| format!("保存客户端检测设置失败：{error}"))?;
    Ok(GameDetectionSettings {
        enabled: input.enabled,
    })
}

#[tauri::command]
pub fn show_champion_rank_overlay(
    payload: crate::domain::GameState,
    app: AppHandle,
) -> Result<(), String> {
    if !crate::domain::should_show_rank_overlay(&payload) {
        return hide_champion_rank_overlay(app);
    }

    let window = app
        .get_webview_window("champion-rank-overlay")
        .ok_or_else(|| "英雄排名浮层窗口未初始化。".to_string())?;
    if !position_rank_overlay(&window)? { return hide_champion_rank_overlay(app); }
    window
        .set_ignore_cursor_events(true)
        .map_err(|error| format!("设置排名浮层鼠标穿透失败：{error}"))?;
    window
        .show()
        .map_err(|error| format!("显示英雄排名浮层失败：{error}"))?;
    window
        .emit("champion-rank:update", payload)
        .map_err(|error| format!("发送英雄排名数据失败：{error}"))
}

#[tauri::command]
pub fn hide_champion_rank_overlay(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("champion-rank-overlay") {
        window
            .hide()
            .map_err(|error| format!("隐藏英雄排名浮层失败：{error}"))?;
    }
    Ok(())
}

#[cfg(windows)]
fn position_rank_overlay(window: &tauri::WebviewWindow) -> Result<bool, String> {
    use std::ffi::c_void;
    #[repr(C)] struct Rect { left: i32, top: i32, right: i32, bottom: i32 }
    #[repr(C)] struct Point { x: i32, y: i32 }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn FindWindowW(class: *const u16, title: *const u16) -> *mut c_void;
        fn GetForegroundWindow() -> *mut c_void;
        fn GetClientRect(hwnd: *mut c_void, rect: *mut Rect) -> i32;
        fn ClientToScreen(hwnd: *mut c_void, point: *mut Point) -> i32;
    }
    let title: Vec<u16> = "League of Legends\0".encode_utf16().collect();
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    if hwnd.is_null() || unsafe { GetForegroundWindow() } != hwnd { return Ok(false); }
    let mut rect = Rect { left: 0, top: 0, right: 0, bottom: 0 };
    let mut origin = Point { x: 0, y: 0 };
    if unsafe { GetClientRect(hwnd, &mut rect) } == 0 || unsafe { ClientToScreen(hwnd, &mut origin) } == 0 || rect.right <= 0 || rect.bottom <= 0 { return Ok(false); }
    window.set_size(Size::Physical(PhysicalSize::new(rect.right as u32, rect.bottom as u32)))
        .map_err(|e| format!("调整英雄排名浮层大小失败：{e}"))?;
    window.set_position(Position::Physical(PhysicalPosition::new(origin.x, origin.y)))
        .map_err(|e| format!("定位英雄排名浮层失败：{e}"))?;
    Ok(true)
}

#[cfg(not(windows))]
fn position_rank_overlay(_: &tauri::WebviewWindow) -> Result<bool, String> { Ok(false) }
