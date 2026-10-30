use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GamePhase {
    Disconnected,
    ChampSelect,
    Loading,
    InGame,
    AugmentChoice,
}
