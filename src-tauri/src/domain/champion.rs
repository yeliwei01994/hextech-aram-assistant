use super::ChampionRankRow;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct ChampionDetail {
    pub champion_id: String,
    pub name: String,
    pub portrait: Option<String>,
    pub ranking: Option<ChampionRankRow>,
    pub patch: Option<String>,
}
