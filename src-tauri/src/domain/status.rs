use super::GamePhase;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct DataStatus {
    pub ready: bool,
    pub source: Option<String>,
    pub patch: Option<String>,
    pub updated_at: Option<String>,
    pub stale: bool,
    pub augment_ready: bool,
    pub augment_source: Option<String>,
    pub augment_patch: Option<String>,
    pub augment_updated_at: Option<String>,
    pub augment_stale: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct AppStatus {
    pub version: String,
    pub data_status: DataStatus,
    pub game_phase: GamePhase,
}
