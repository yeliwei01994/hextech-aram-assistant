use crate::{db::map_rank_row, domain::ChampionRankRow, AppState};
use tauri::State;

#[tauri::command]
pub fn list_hero_rankings(
    query: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ChampionRankRow>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "本地数据库锁定失败".to_string())?;
    let query = query.unwrap_or_default();
    let mut statement = db.prepare(
        "SELECT c.champion_id, COALESCE(r.name, c.name), r.position, r.total, r.badge_tier, r.patch, r.source, r.updated_at
         FROM champions c
         LEFT JOIN champion_rankings r ON r.champion_id = c.champion_id
         WHERE (r.champion_id IS NOT NULL OR NOT EXISTS (SELECT 1 FROM champion_rankings))
           AND (c.name LIKE '%' || ?1 || '%' OR c.champion_id LIKE '%' || ?1 || '%')
         ORDER BY CASE WHEN r.position IS NULL THEN 1 ELSE 0 END, r.position ASC, c.name COLLATE NOCASE ASC"
    ).map_err(|error| format!("准备榜单查询失败：{error}"))?;

    let rows = statement
        .query_map([query], map_rank_row)
        .map_err(|error| format!("查询榜单失败：{error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取榜单记录失败：{error}"))
}
