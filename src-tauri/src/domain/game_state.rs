use serde::{Deserialize, Serialize};

use super::{ChampionRankRow, GamePhase};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GameState {
    pub enabled: bool,
    pub connected: bool,
    pub phase: GamePhase,
    pub source: Option<String>,
    pub player_champion_id: Option<String>,
    pub player_champion_name: Option<String>,
    pub ranking: Option<ChampionRankRow>,
    #[serde(default)]
    pub bench: Vec<Option<ChampionRankRow>>,
}

pub fn should_show_rank_overlay(state: &GameState) -> bool {
    state.enabled
        && state.connected
        && state.phase == GamePhase::ChampSelect
        && (!state.bench.is_empty() || state
            .player_champion_id
            .as_deref()
            .is_some_and(|champion_id| !champion_id.trim().is_empty()))
}

#[derive(Clone, Debug, Serialize)]
pub struct GameDetectionSettings {
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GameDetectionSettingsInput {
    pub enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::{should_show_rank_overlay, GamePhase, GameState};

    fn state(phase: GamePhase, connected: bool, champion_id: Option<&str>) -> GameState {
        GameState {
            enabled: true,
            connected,
            phase,
            source: Some("lcu".to_string()),
            player_champion_id: champion_id.map(str::to_string),
            player_champion_name: champion_id.map(str::to_string),
            ranking: None,
            bench: Vec::new(),
        }
    }

    #[test]
    fn shows_rank_overlay_for_selected_champion_even_without_cached_rank() {
        let game = state(GamePhase::ChampSelect, true, Some("Evelynn"));
        assert!(should_show_rank_overlay(&game));
    }

    #[test]
    fn hides_rank_overlay_when_selection_is_not_active_or_champion_is_unknown() {
        assert!(!should_show_rank_overlay(&state(
            GamePhase::InGame,
            true,
            Some("Evelynn")
        )));
        assert!(!should_show_rank_overlay(&state(
            GamePhase::ChampSelect,
            true,
            None
        )));
        assert!(!should_show_rank_overlay(&state(
            GamePhase::ChampSelect,
            false,
            Some("Evelynn")
        )));
    }

    #[test]
    fn hides_rank_overlay_when_detection_is_disabled() {
        let mut game = state(GamePhase::ChampSelect, true, Some("Evelynn"));
        game.enabled = false;
        assert!(!should_show_rank_overlay(&game));
    }
}
