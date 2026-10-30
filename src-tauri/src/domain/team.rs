use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreatTag {
    Physical,
    Magic,
    Healing,
    Shielding,
    Durable,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TeamBuildInput {
    pub player_champion_id: String,
    #[serde(default)]
    pub allies: Vec<String>,
    #[serde(default)]
    pub enemies: Vec<String>,
    pub allies_complete: bool,
    pub enemies_complete: bool,
    #[serde(default)]
    pub enemy_tags: Vec<ThreatTag>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BuildSuggestion {
    pub item_id: String,
    pub item_name: String,
    pub priority: u8,
    pub reasons: Vec<String>,
    pub source: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct BuildAdvice {
    pub allies_complete: bool,
    pub enemies_complete: bool,
    pub suggestions: Vec<BuildSuggestion>,
    pub inferred_enemy_tags: Vec<ThreatTag>,
    pub ai_status: String,
    pub ai_summary: Option<String>,
}
