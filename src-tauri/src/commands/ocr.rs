use crate::{
    domain::{AugmentGradeRow, OcrCardAdvice, OcrOverlayPayload, OcrScreenBounds},
    ocr::{is_safe_name_match_for_overlay, match_augment_name, OcrScreenResult},
    AppState,
};
use rusqlite::OptionalExtension;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewWindow};

#[tauri::command]
pub async fn recognize_current_screen(
    champion_id: Option<String>,
    only_if_augment_choice: bool,
    app: AppHandle,
) -> Result<Option<OcrOverlayPayload>, String> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        recognize_impl(&app, champion_id, only_if_augment_choice)
    })
    .await
    .map_err(|error| format!("OCR 任务异常退出：{error}"))?;
    if let Err(error) = &result { eprintln!("海克斯识别失败：{error}"); }
    result
}

fn recognize_impl(
    app: &AppHandle,
    champion_id: Option<String>,
    only_if_augment_choice: bool,
) -> Result<Option<OcrOverlayPayload>, String> {
    let state = app.state::<AppState>();
    let screen = {
        let mut ocr = state
            .ocr
            .lock()
            .map_err(|_| "OCR 识别器暂不可用，请重启应用。".to_string())?;
        let started = Instant::now();
        let screen = ocr.recognize_current_screen()?;
        if only_if_augment_choice {
            let probe = ocr.probe_current_screen()?;
            if !probe.game_foreground || !probe.augment_choice {
                eprintln!("海克斯识别：识别完成后卡片界面已离开");
                return Ok(None);
            }
        }
        eprintln!("海克斯识别：截图、OCR 与界面校验耗时 {} ms", started.elapsed().as_millis());
        screen
    };

    let db = state
        .db
        .lock()
        .map_err(|_| "本地数据库锁定失败".to_string())?;
    let (champion_name, grade_rows) = if let Some(id) = champion_id.as_deref() {
        let name = db
            .query_row(
                "SELECT name FROM champions WHERE champion_id = ?1 COLLATE NOCASE OR name = ?1 COLLATE NOCASE LIMIT 1",
                [id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("读取当前英雄失败：{error}"))?;
        let grades = load_grades(&db, id)?;
        (name, grades)
    } else {
        (None, Vec::new())
    };

    let cards = make_advice_cards(&screen, &grade_rows);
    eprintln!("海克斯识别：英雄={}，读到 {} 张卡片，匹配到 {} 个等级", champion_name.as_deref().unwrap_or("未知"), cards.len(), cards.iter().filter(|card| card.grade.is_some()).count());
    let message = if champion_name.is_none() {
        "已读取卡片。选择当前英雄后可显示对应的 S / A / B 分级。".to_string()
    } else if grade_rows.is_empty() {
        "已识别卡片，但本地还没有该英雄的海克斯分级数据。".to_string()
    } else if cards.iter().all(|card| card.raw_text.is_empty()) {
        "暂时没读清卡片标题，请确认海克斯选择界面在前台后重试。".to_string()
    } else if grade_rows.first().is_some_and(|row| row.reason == "OP.GG 该英雄海克斯等级") {
        format!("OP.GG 海克斯等级 · {} · OCR 结果仅供参考", grade_rows.first().and_then(|row| row.patch.as_deref()).unwrap_or("版本未知"))
    } else {
        format!("本地海克斯缓存 · {} · OCR 结果仅供参考", grade_rows.first().and_then(|row| row.patch.as_deref()).unwrap_or("版本未核实"))
    };

    Ok(Some(OcrOverlayPayload {
        champion_name,
        message,
        monitor: screen.monitor,
        game_window: screen.game_window,
        cards,
    }))
}

fn load_grades(
    db: &rusqlite::Connection,
    champion_id: &str,
) -> Result<Vec<AugmentGradeRow>, String> {
    let mut statement = db
        .prepare(
            "SELECT champion_id, augment_id, name, grade, reason, patch FROM champion_augment_grades WHERE champion_id = ?1 COLLATE NOCASE ORDER BY CASE grade WHEN 'S' THEN 0 WHEN 'A' THEN 1 ELSE 2 END, name COLLATE NOCASE",
        )
        .map_err(|error| format!("准备海克斯查询失败：{error}"))?;
    let rows = statement
        .query_map([champion_id], |row| {
            Ok(AugmentGradeRow {
                champion_id: row.get(0)?,
                augment_id: row.get(1)?,
                name: row.get(2)?,
                grade: row.get(3)?,
                reason: row.get(4)?,
                patch: row.get(5)?,
            })
        })
        .map_err(|error| format!("查询海克斯建议失败：{error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取海克斯建议失败：{error}"))
}

fn make_advice_cards(screen: &OcrScreenResult, grades: &[AugmentGradeRow]) -> Vec<OcrCardAdvice> {
    let known_names: Vec<String> = grades.iter().map(|row| row.name.clone()).collect();
    screen
        .cards
        .iter()
        .map(|card| {
            let best = match_augment_name(&card.raw_text, &known_names);
            let matched_name = best
                .as_ref()
                .filter(|candidate| {
                    is_safe_name_match_for_overlay(candidate.similarity, card.confidence)
                })
                .map(|candidate| candidate.name.to_string());
            let candidate_name = best.map(|candidate| candidate.name.to_string());
            let row = matched_name
                .as_deref()
                .and_then(|name| grades.iter().find(|row| row.name == name));
            OcrCardAdvice {
                slot: card.slot,
                raw_text: card.raw_text.clone(),
                confidence: if card.confidence.is_finite() {
                    card.confidence.clamp(0.0, 1.0)
                } else {
                    0.0
                },
                matched_name,
                candidate_name,
                grade: row.map(|row| row.grade.clone()),
                reason: row.map(|row| row.reason.clone()),
            }
        })
        .collect()
}

#[tauri::command]
pub fn show_ocr_overlay(payload: OcrOverlayPayload, app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("ocr-overlay")
        .ok_or_else(|| "海克斯浮层窗口未初始化。".to_string())?;
    position_overlay(
        &window,
        payload.monitor.as_ref(),
        payload.game_window.as_ref(),
    )?;
    window
        .set_ignore_cursor_events(true)
        .map_err(|error| format!("设置浮层鼠标穿透失败：{error}"))?;
    window
        .show()
        .map_err(|error| format!("显示海克斯浮层失败：{error}"))?;
    window.emit("ocr:results", payload)
        .map_err(|error| format!("发送海克斯建议失败：{error}"))?;
    eprintln!("海克斯浮层：已显示并发送结果");
    Ok(())
}

#[tauri::command]
pub fn hide_ocr_overlay(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("ocr-overlay") {
        window
            .hide()
            .map_err(|error| format!("关闭海克斯浮层失败：{error}"))?;
    }
    Ok(())
}

fn position_overlay(
    window: &WebviewWindow,
    captured_monitor: Option<&OcrScreenBounds>,
    game_window: Option<&OcrScreenBounds>,
) -> Result<(), String> {
    let (x, y, width, height) = if let Some(bounds) = game_window.or(captured_monitor) {
        (bounds.left, bounds.top, bounds.width, bounds.height)
    } else if let Some(monitor) = window.primary_monitor().map_err(|error| format!("读取主显示器失败：{error}"))? {
        (monitor.position().x, monitor.position().y, monitor.size().width, monitor.size().height)
    } else { return Err("找不到游戏窗口或显示器".to_string()); };
    window.set_size(Size::Physical(PhysicalSize::new(width, height)))
        .map_err(|error| format!("调整海克斯浮层大小失败：{error}"))?;
    window.set_position(Position::Physical(PhysicalPosition::new(x, y)))
        .map_err(|error| format!("定位海克斯浮层失败：{error}"))
}
