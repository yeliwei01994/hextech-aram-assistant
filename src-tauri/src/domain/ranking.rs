use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChampionRankRow {
    pub champion_id: String,
    pub name: String,
    pub position: Option<u16>,
    pub total: Option<u16>,
    pub badge_tier: Option<u8>,
    pub patch: Option<String>,
    pub source: Option<String>,
    pub updated_at: Option<String>,
}
