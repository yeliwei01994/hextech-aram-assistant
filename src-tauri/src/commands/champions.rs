use crate::{
    db::map_rank_row,
    domain::{AugmentGradeRow, ChampionDetail},
    AppState,
};
use rusqlite::OptionalExtension;
use tauri::State;

#[tauri::command]
pub fn get_hero_detail(
    champion_id: String,
    state: State<'_, AppState>,
) -> Result<Option<ChampionDetail>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "本地数据库锁定失败".to_string())?;
    let champion = db
        .query_row(
            "SELECT champion_id, name, portrait, patch FROM champions WHERE champion_id = ?1",
            [&champion_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|error| format!("查询英雄失败：{error}"))?;

    let Some((id, name, portrait, patch)) = champion else {
        return Ok(None);
    };
    let ranking = db.query_row(
        "SELECT champion_id, name, position, total, badge_tier, patch, source, updated_at FROM champion_rankings WHERE champion_id = ?1",
        [&champion_id], map_rank_row,
    ).optional().map_err(|error| format!("查询英雄排名失败：{error}"))?;

    Ok(Some(ChampionDetail {
        champion_id: id,
        name,
        portrait,
        ranking,
        patch,
    }))
}

#[tauri::command]
pub fn get_augment_grades(
    champion_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<AugmentGradeRow>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "本地数据库锁定失败".to_string())?;
    let mut statement = db.prepare(
        "SELECT champion_id, augment_id, name, grade, reason, patch FROM champion_augment_grades WHERE champion_id = ?1 ORDER BY CASE grade WHEN 'S' THEN 0 WHEN 'A' THEN 1 ELSE 2 END, name COLLATE NOCASE"
    ).map_err(|error| format!("准备海克斯查询失败：{error}"))?;
    let rows = statement
        .query_map([champion_id], |row| {
            let grade_text: String = row.get(3)?;
            let grade = match grade_text.as_str() {
                "S" => "S",
                "A" => "A",
                "B" => "B",
                _ => return Err(rusqlite::Error::InvalidQuery),
            };
            Ok(AugmentGradeRow {
                champion_id: row.get(0)?,
                augment_id: row.get(1)?,
                name: row.get(2)?,
                grade: grade.to_string(),
                reason: row.get(4)?,
                patch: row.get(5)?,
            })
        })
        .map_err(|error| format!("查询海克斯等级失败：{error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取海克斯记录失败：{error}"))
}
