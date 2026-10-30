use super::ThreatTag;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
pub struct AiSettingsInput {
    pub endpoint: String,
    pub model: String,
    pub enabled: bool,
    pub api_key: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct AiSettingsView {
    pub endpoint: String,
    pub model: String,
    pub enabled: bool,
    pub key_configured: bool,
}

#[derive(Clone, Debug)]
pub struct AiExplanation {
    pub inferred_enemy_tags: Vec<ThreatTag>,
}
