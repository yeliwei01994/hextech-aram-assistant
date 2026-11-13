use crate::{
    domain::{BuildAdvice, TeamBuildInput},
    providers::ai,
    recommendation::recommend,
    AppState,
};
use tauri::State;
use rusqlite::OptionalExtension;
use serde_json::{json, Value};

#[tauri::command]
pub async fn get_live_shop_context(state: State<'_, AppState>) -> Result<Option<Value>, String> {
    if !super::game::get_game_detection_settings(state)?.enabled {
        return Ok(None);
    }
    Ok(crate::game::watcher::live_shop_context().await)
}

#[tauri::command]
pub fn get_equipment_stats(champion: String, state: State<'_, AppState>) -> Result<Option<Value>, String> {
    let db = state.db.lock().map_err(|_| "数据库锁定失败")?;
    let Some((id, name)) = crate::db::resolve_champion_identity(&db, champion.trim(), None)
        .map_err(|e| e.to_string())? else { return Ok(None) };
    let source_id: Option<String> = db.query_row(
        "SELECT source_id FROM aramhex_heroes WHERE champion_id=?1", [&id], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    let cached: Option<String> = db.query_row(
        "SELECT value FROM app_settings WHERE key='aramhex_equipment'", [], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    drop(db);
    let (Some(source_id), Some(cached)) = (source_id, cached) else { return Ok(None) };
    let snapshot: Value = serde_json::from_str(&cached).map_err(|e| e.to_string())?;
    let Some(items) = snapshot["hero_items.json"][&source_id].as_object() else { return Ok(None) };
    let total = snapshot["equipment_heroes.json"][&source_id]["games"].as_u64();
    let metadata = &snapshot["item_metadata.json"];
    let mut components = std::collections::HashMap::<&str, Vec<&str>>::new();
    if let Some(entries) = metadata.as_object() {
        for (component, item) in entries {
            for target in item["into"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                components.entry(target).or_default().push(component.as_str());
            }
        }
    }
    let mut rows = Vec::new();
    for (item_id, stats) in items {
        let (Some(games), Some(wins), Some(name)) = (
            stats["games"].as_u64(), stats["wins"].as_u64(), metadata[item_id]["name"].as_str(),
        ) else { continue };
        if games < 300 || wins > games || total.is_some_and(|total| games > total)
            || stats["low_sample"].as_bool() == Some(true) { continue; }
        rows.push((games, json!({
            "id": item_id, "name": name, "games": games, "winRate": wins as f64 / games as f64,
            "holdingRate": total.map(|total| games as f64 / total as f64),
            "price": metadata[item_id]["total"].as_u64(),
            "components": components.get(item_id.as_str()),
            "componentOptions": components.get(item_id.as_str()).into_iter().flatten()
                .filter_map(|id| {
                    let item = &metadata[*id];
                    let price = item["total"].as_u64().filter(|price| *price > 0)?;
                    Some(json!({"name": item["name"].as_str()?, "price": price}))
                }).collect::<Vec<_>>(),
        })));
    }
    rows.sort_by_key(|(games, _)| std::cmp::Reverse(*games));
    let routes: Value = serde_json::from_str(include_str!("../../../data/seed/build-routes.json"))
        .map_err(|e| e.to_string())?;
    let route = &routes[&source_id];
    let starters: Vec<Vec<Value>> = route["starters"].as_array().into_iter().flatten()
        .filter_map(|group| group.as_array()?.iter().map(|id| {
            let item = &metadata[id.as_str()?];
            let price = item["total"].as_u64().filter(|price| *price > 0)?;
            Some(json!({"id": id, "name": item["name"].as_str()?, "price": price}))
        }).collect()).collect();
    Ok(Some(json!({"patch": snapshot["patch"], "champion": name,
        "route": route, "starters": starters,
        "items": rows.into_iter().map(|(_, row)| row).collect::<Vec<_>>()})))
}

#[tauri::command]
pub async fn recommend_build(
    input: TeamBuildInput,
    state: State<'_, AppState>,
) -> Result<BuildAdvice, String> {
    let mut advice = recommend(input.clone());
    let settings = {
        let db = state
            .db
            .lock()
            .map_err(|_| "本地数据库锁定失败".to_string())?;
        ai::get_settings(&db)
    };

    if !settings.enabled {
        advice.ai_status = "disabled".to_string();
        return Ok(advice);
    }
    let has_enemy_names = input.enemies.iter().any(|name| !name.trim().is_empty());
    if !has_enemy_names {
        if advice.suggestions.is_empty() {
            advice.ai_status = "no_candidates".to_string();
        }
        return Ok(advice);
    }
    match ai::analyze_team(&settings.endpoint, &settings.model, &input).await {
        Ok(explanation) => {
            let inferred_enemy_tags: Vec<_> = explanation
                .inferred_enemy_tags
                .into_iter()
                .filter(|tag| !input.enemy_tags.contains(tag))
                .collect();
            let base_ids: std::collections::HashSet<String> = advice
                .suggestions
                .iter()
                .map(|suggestion| suggestion.item_id.clone())
                .collect();
            let mut merged_input = input.clone();
            for tag in &inferred_enemy_tags {
                if !merged_input.enemy_tags.contains(tag) {
                    merged_input.enemy_tags.push(tag.clone());
                }
            }
            let mut merged = recommend(merged_input);
            for suggestion in &mut merged.suggestions {
                if !base_ids.contains(&suggestion.item_id) {
                    suggestion.source = "ai".to_string();
                    suggestion
                        .reasons
                        .push("AI 根据敌方英雄推断出该阵容特征".to_string());
                }
            }
            merged.inferred_enemy_tags = inferred_enemy_tags;
            merged.ai_status = "complete".to_string();
            merged.ai_summary = Some(if merged.inferred_enemy_tags.is_empty() {
                "未从敌方英雄名称中识别到明确威胁；保留手动标签和离线建议。".to_string()
            } else {
                format!(
                    "识别到{}；装备方向由本地规则映射。",
                    merged
                        .inferred_enemy_tags
                        .iter()
                        .map(threat_label)
                        .collect::<Vec<_>>()
                        .join("、")
                )
            });
            advice = merged;
        }
        Err(_) => advice.ai_status = "unavailable".to_string(),
    }
    Ok(advice)
}

fn threat_label(tag: &crate::domain::ThreatTag) -> &'static str {
    match tag {
        crate::domain::ThreatTag::Physical => "物理伤害较高",
        crate::domain::ThreatTag::Magic => "魔法伤害较高",
        crate::domain::ThreatTag::Healing => "回复能力较强",
        crate::domain::ThreatTag::Shielding => "护盾较多",
        crate::domain::ThreatTag::Durable => "前排较硬",
    }
}
