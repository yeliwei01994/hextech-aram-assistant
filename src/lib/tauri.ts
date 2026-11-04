import { invoke } from "@tauri-apps/api/core";
import type {
  AppStatus,
  AiSettingsInput,
  AiSettingsView,
  AugmentGradeRow,
  BuildAdvice,
  ChampionDetail,
  ChampionRankRow,
  DataStatus,
  GameDetectionSettings,
  GameState,
  OcrOverlayPayload,
  TeamBuildInput,
} from "../types/domain";

const isTauri = () => "__TAURI_INTERNALS__" in window;

export async function getAppStatus(): Promise<AppStatus> {
  if (isTauri()) return invoke<AppStatus>("get_app_status");
  return {
    version: "0.1.0",
    data_status: { ready: false, source: null, patch: null, updated_at: null, stale: false, augment_ready: false, augment_source: null, augment_patch: null, augment_updated_at: null, augment_stale: false },
    game_phase: "disconnected",
  };
}

export async function listHeroRankings(query: string): Promise<ChampionRankRow[]> {
  if (isTauri()) return invoke<ChampionRankRow[]>("list_hero_rankings", { query: query || null });
  return [];
}

export async function getHeroDetail(championId: string): Promise<ChampionDetail | null> {
  if (isTauri()) return invoke<ChampionDetail | null>("get_hero_detail", { championId });
  return null;
}

export async function getAugmentGrades(championId: string): Promise<AugmentGradeRow[]> {
  if (isTauri()) return invoke<AugmentGradeRow[]>("get_augment_grades", { championId });
  return [];
}

export async function recommendBuild(input: TeamBuildInput): Promise<BuildAdvice> {
  if (isTauri()) return invoke<BuildAdvice>("recommend_build", { input });
  return { allies_complete: input.allies_complete, enemies_complete: input.enemies_complete, suggestions: [], inferred_enemy_tags: [], ai_status: "disabled", ai_summary: null };
}

export async function importRankingsJson(contents: string): Promise<DataStatus> {
  if (!isTauri()) throw new Error("请在 Windows 桌面应用中导入数据。");
  return invoke<DataStatus>("import_rankings_json", { contents });
}

export async function importAugmentGradesJson(contents: string): Promise<number> {
  if (!isTauri()) throw new Error("请在 Windows 桌面应用中导入数据。");
  return invoke<number>("import_augment_grades_json", { contents });
}

export async function getAiSettings(): Promise<AiSettingsView> {
  if (isTauri()) return invoke<AiSettingsView>("get_ai_settings");
  return { endpoint: "https://api.openai.com/v1", model: "", enabled: false, key_configured: false };
}

export async function saveAiSettings(input: AiSettingsInput): Promise<AiSettingsView> {
  if (!isTauri()) throw new Error("AI 设置仅能在 Windows 桌面应用中保存。");
  return invoke<AiSettingsView>("save_ai_settings", { input });
}

export async function getGameState(): Promise<GameState> {
  if (isTauri()) return invoke<GameState>("get_game_state");
  return {
    enabled: false,
    connected: false,
    phase: "disconnected",
    source: null,
    player_champion_id: null,
    player_champion_name: null,
    ranking: null,
    bench: [],
  };
}

export async function getGameDetectionSettings(): Promise<GameDetectionSettings> {
  if (isTauri()) return invoke<GameDetectionSettings>("get_game_detection_settings");
  return { enabled: false };
}

export async function saveGameDetectionSettings(enabled: boolean): Promise<GameDetectionSettings> {
  if (!isTauri()) throw new Error("客户端检测设置仅能在 Windows 桌面应用中保存。");
  return invoke<GameDetectionSettings>("save_game_detection_settings", { input: { enabled } });
}

export async function getNotificationPermission(): Promise<"granted" | "denied" | "prompt"> {
  if (isTauri()) return invoke("get_notification_permission");
  return "prompt";
}

export async function requestNotificationPermission(): Promise<"granted" | "denied" | "prompt"> {
  if (isTauri()) return invoke("request_notification_permission");
  return "prompt";
}

export async function sendAppNotification(title: string, body: string): Promise<boolean> {
  if (!isTauri()) return false;
  try {
    return await invoke<boolean>("send_app_notification", { title, body });
  } catch (error) {
    console.warn("发送 Windows 通知失败", error);
    return false;
  }
}

export async function showChampionRankOverlay(state: GameState): Promise<void> {
  if (!isTauri()) return;
  return invoke<void>("show_champion_rank_overlay", { payload: state });
}

export async function hideChampionRankOverlay(): Promise<void> {
  if (!isTauri()) return;
  return invoke<void>("hide_champion_rank_overlay");
}

export async function recognizeCurrentScreen(championId: string | null, onlyIfAugmentChoice: boolean): Promise<OcrOverlayPayload | null> {
  if (!isTauri()) throw new Error("屏幕 OCR 仅能在 Windows 桌面应用中使用。");
  return invoke<OcrOverlayPayload | null>("recognize_current_screen", { championId, onlyIfAugmentChoice });
}

export async function showOcrOverlay(payload: OcrOverlayPayload): Promise<void> {
  if (!isTauri()) return;
  return invoke<void>("show_ocr_overlay", { payload });
}

export async function hideOcrOverlay(): Promise<void> {
  if (!isTauri()) return;
  return invoke<void>("hide_ocr_overlay");
}
