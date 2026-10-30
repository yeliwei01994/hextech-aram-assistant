use crate::domain::{BuildAdvice, BuildSuggestion, TeamBuildInput, ThreatTag};

pub fn recommend(input: TeamBuildInput) -> BuildAdvice {
    let mut suggestions = Vec::new();
    let mut add = |tag: &str, name: &str, reason: &str| {
        suggestions.push(BuildSuggestion {
            item_id: tag.to_string(),
            item_name: name.to_string(),
            priority: suggestions.len() as u8 + 1,
            reasons: vec![reason.to_string()],
            source: "local_rule".to_string(),
        });
    };

    for tag in &input.enemy_tags {
        match tag {
            ThreatTag::Physical => add("armor", "护甲类装备", "敌方物理伤害较多，优先补充护甲"),
            ThreatTag::Magic => add(
                "magic_resist",
                "魔抗类装备",
                "敌方魔法伤害较多，优先补充魔抗",
            ),
            ThreatTag::Healing => add(
                "anti_heal",
                "重伤类装备",
                "敌方回复能力较强，可考虑重伤效果",
            ),
            ThreatTag::Shielding => add(
                "anti_shield",
                "护盾克制装备",
                "敌方护盾较多，可考虑护盾克制效果",
            ),
            ThreatTag::Durable => add(
                "penetration",
                "穿透类装备",
                "敌方前排较硬，可考虑物理或法术穿透",
            ),
        }
    }

    BuildAdvice {
        allies_complete: input.allies_complete,
        enemies_complete: input.enemies_complete,
        suggestions,
        inferred_enemy_tags: Vec::new(),
        ai_status: "disabled".to_string(),
        ai_summary: None,
    }
}
