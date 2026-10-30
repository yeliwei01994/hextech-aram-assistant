use crate::{
    domain::{BuildAdvice, TeamBuildInput},
    providers::ai,
    recommendation::recommend,
    AppState,
};
use tauri::State;

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
