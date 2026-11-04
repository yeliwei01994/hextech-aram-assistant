export type GamePhase = "disconnected" | "champ_select" | "loading" | "in_game" | "augment_choice";

export interface DataStatus {
  ready: boolean;
  source: string | null;
  patch: string | null;
  updated_at: string | null;
  stale: boolean;
  augment_ready: boolean;
  augment_source: string | null;
  augment_patch: string | null;
  augment_updated_at: string | null;
  augment_stale: boolean;
}

export interface AppStatus {
  version: string;
  data_status: DataStatus;
  game_phase: GamePhase;
}

export interface GameState {
  enabled: boolean;
  connected: boolean;
  phase: GamePhase;
  source: "lcu" | "live_client" | null;
  player_champion_id: string | null;
  player_champion_name: string | null;
  ranking: ChampionRankRow | null;
  bench: (ChampionRankRow | null)[];
}

export interface GameDetectionSettings {
  enabled: boolean;
}

export interface OcrScreenBounds {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface OcrCardAdvice {
  slot: number;
  raw_text: string;
  confidence: number;
  matched_name: string | null;
  candidate_name: string | null;
  grade: "S" | "A" | "B" | null;
  reason: string | null;
  win_rate?: number | null;
  games?: number | null;
  low_sample?: boolean;
}

export interface OcrOverlayPayload {
  champion_name: string | null;
  message: string;
  monitor: OcrScreenBounds | null;
  game_window: OcrScreenBounds | null;
  cards: OcrCardAdvice[];
}

export interface SnapshotImportResult {
  records: number;
  status: DataStatus | null;
}

export interface ChampionRankRow {
  champion_id: string;
  name: string;
  position: number | null;
  total: number | null;
  badge_tier: number | null;
  patch: string | null;
  source: string | null;
  updated_at: string | null;
}

export interface ChampionDetail {
  champion_id: string;
  name: string;
  portrait: string | null;
  ranking: ChampionRankRow | null;
  patch: string | null;
}

export interface AugmentGradeRow {
  champion_id: string;
  augment_id: string;
  name: string;
  grade: "S" | "A" | "B" | null;
  reason: string;
  patch: string | null;
  win_rate: number | null;
  games: number | null;
  delta_pp: number | null;
  low_sample: boolean;
}

export type ThreatTag = "physical" | "magic" | "healing" | "shielding" | "durable";

export interface TeamBuildInput {
  player_champion_id: string;
  allies: string[];
  enemies: string[];
  allies_complete: boolean;
  enemies_complete: boolean;
  enemy_tags: ThreatTag[];
}

export interface BuildSuggestion {
  item_id: string;
  item_name: string;
  priority: number;
  reasons: string[];
  source: "local_rule" | "ai";
}

export interface BuildAdvice {
  allies_complete: boolean;
  enemies_complete: boolean;
  suggestions: BuildSuggestion[];
  inferred_enemy_tags: ThreatTag[];
  ai_status: "disabled" | "no_candidates" | "complete" | "unavailable";
  ai_summary: string | null;
}

export interface AiSettingsView {
  endpoint: string;
  model: string;
  enabled: boolean;
  key_configured: boolean;
}

export interface AiSettingsInput {
  endpoint: string;
  model: string;
  enabled: boolean;
  api_key: string;
}

export interface RuleTag {
  id: ThreatTag;
  label: string;
  hint: string;
}
