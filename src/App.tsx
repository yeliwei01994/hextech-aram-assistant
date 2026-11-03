import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  ArrowDownToLine,
  CircleHelp,
  Crosshair,
  Database,
  Shield,
  Sparkles,
  Wifi,
  WifiOff,
} from "lucide-react";
import { getAiSettings, getAppStatus, getGameDetectionSettings, getGameState, hideOcrOverlay, importAugmentGradesJson, importRankingsJson, recognizeCurrentScreen, refreshChampionAugments, saveAiSettings, saveGameDetectionSettings, showOcrOverlay } from "./lib/tauri";
import { getNotificationPermission, requestNotificationPermission, sendAppNotification } from "./lib/notifications";
import type { AiSettingsView, AppStatus, GameDetectionSettings, GameState } from "./types/domain";

const EMPTY_GAME_STATE: GameState = {
  enabled: true,
  connected: false,
  phase: "disconnected",
  source: null,
  player_champion_id: null,
  player_champion_name: null,
  ranking: null,
  bench: [],
};

function App() {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [gameState, setGameState] = useState<GameState>(EMPTY_GAME_STATE);
  const [statusError, setStatusError] = useState(false);
  const [refreshKey, setRefreshKey] = useState(0);
  const gameStateRef = useRef(gameState);
  const connectedNotified = useRef(false);
  const notificationsReady = useRef(false);
  const pendingConnectionNotification = useRef(false);
  const ocrBusy = useRef(false);
  const pendingAutoRecognition = useRef(false);
  const autoDetectionActive = useRef(false);
  const autoDismissedUntilExit = useRef(false);
  const refreshedAugments = useRef(new Set<string>());
  gameStateRef.current = gameState;

  useEffect(() => {
    getAppStatus().then((nextStatus) => { setStatus(nextStatus); setStatusError(false); }).catch(() => setStatusError(true));
  }, [refreshKey]);

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let disposed = false;
    const unlisten: Array<() => void> = [];
    const register = <T,>(name: string, handler: (payload: T) => void) => {
      void listen<T>(name, (event) => handler(event.payload)).then((stop) => {
        if (disposed) stop();
        else unlisten.push(stop);
      }).catch((error) => console.warn(`注册后台事件 ${name} 失败`, error));
    };
    const applyGameState = (next: GameState) => {
      gameStateRef.current = next;
      setGameState(next);
      if (!next.enabled || !next.connected) {
        connectedNotified.current = false;
        pendingConnectionNotification.current = false;
        return;
      }
      pendingConnectionNotification.current = true;
      if (notificationsReady.current && !connectedNotified.current) {
        connectedNotified.current = true;
        pendingConnectionNotification.current = false;
        void sendAppNotification("已连接英雄联盟", "本地识别已就绪，选人排名与海克斯提示会自动显示。");
      }
    };
    const recognize = (automatic: boolean) => {
      if (automatic && autoDismissedUntilExit.current) return;
      if (ocrBusy.current) {
        if (automatic) pendingAutoRecognition.current = true;
        return;
      }
      ocrBusy.current = true;
      void (async () => {
        const championId = gameStateRef.current.player_champion_id;
        try {
          const payload = await recognizeCurrentScreen(championId, automatic);
          if (!payload || (automatic && (!autoDetectionActive.current || autoDismissedUntilExit.current))) return;
          await showOcrOverlay(payload);
        } catch (cause) {
          if (automatic) return;
          await showOcrOverlay({
            champion_name: gameStateRef.current.player_champion_name || null,
            message: cause instanceof Error ? cause.message : String(cause),
            monitor: null,
            game_window: null,
            cards: [],
          }).catch(() => undefined);
        } finally {
          ocrBusy.current = false;
          if (pendingAutoRecognition.current) {
            pendingAutoRecognition.current = false;
            if (autoDetectionActive.current && !autoDismissedUntilExit.current) {
              window.setTimeout(() => recognize(true), 0);
            }
          }
        }
      })();
    };

    register<GameState>("game:state", applyGameState);
    register("ocr:request", () => recognize(false));
    register("ocr:auto-request", () => {
      autoDetectionActive.current = true;
      recognize(true);
    });
    register("game:refresh", () => {
      void getGameState().then(applyGameState).catch(() => undefined);
    });
    register("rankings:updated", () => {
      setRefreshKey((current) => current + 1);
      void getGameState().then(applyGameState).catch(() => undefined);
    });
    register("ocr:hide", () => {
      if (autoDetectionActive.current) autoDismissedUntilExit.current = true;
      pendingAutoRecognition.current = false;
      void hideOcrOverlay().catch(() => undefined);
    });
    register("ocr:auto-hide", () => {
      autoDetectionActive.current = false;
      autoDismissedUntilExit.current = false;
      pendingAutoRecognition.current = false;
      void hideOcrOverlay().catch(() => undefined);
    });

    void (async () => {
      try {
        let permission = await getNotificationPermission();
        if (permission === "prompt") permission = await requestNotificationPermission();
        notificationsReady.current = true;
        if (permission === "granted") {
          await sendAppNotification("海克斯大乱斗小助手", "已在后台运行。右键系统托盘图标可打开设置或退出。");
          if (pendingConnectionNotification.current && !connectedNotified.current) {
            connectedNotified.current = true;
            pendingConnectionNotification.current = false;
            await sendAppNotification("已连接英雄联盟", "本地识别已就绪，选人排名与海克斯提示会自动显示。");
          }
        }
      } catch (error) {
        console.warn("初始化 Windows 通知失败", error);
      }
      try {
        applyGameState(await getGameState());
      } catch (error) {
        console.warn("读取游戏状态失败", error);
      }
    })();

    return () => {
      disposed = true;
      unlisten.forEach((stop) => stop());
    };
  }, []);

  useEffect(() => {
    const championId = gameState.player_champion_id;
    if (!championId || !/^[a-z0-9]+$/i.test(championId) || refreshedAugments.current.has(championId.toLowerCase())) return;
    refreshedAugments.current.add(championId.toLowerCase());
    void refreshChampionAugments(championId).then(() => setRefreshKey((key) => key + 1)).catch((error) => {
      refreshedAugments.current.delete(championId.toLowerCase());
      console.warn("海克斯同步失败，继续使用本地数据", error);
    });
  }, [gameState.player_champion_id]);

  return <div className="app-shell"><main className="main-area">
    <header className="compact-header">
      <div className="brand-row"><div className="brand-mark"><Crosshair size={17} strokeWidth={2.2} /></div><div className="brand-copy"><strong>海克斯大乱斗小助手</strong><span>ARAM 对局助手</span></div></div>
      <div className="connection-card" role="status"><div className={`connection-icon ${gameState.connected ? "connected" : ""}`}>{gameState.connected ? <Wifi size={14} /> : <WifiOff size={14} />}</div><div><strong>{gameState.connected ? "客户端已连接" : gameState.enabled ? "等待客户端" : "自动识别已关闭"}</strong><span>{gameState.player_champion_name || (gameState.enabled ? "自动识别已开启" : "不扫描客户端进程")}</span></div><span className={`connection-dot ${gameState.connected ? "connected" : ""}`} /></div>
    </header>
    <div className="page-wrap"><SettingsPage status={status} gameState={gameState} error={statusError} onImported={() => setRefreshKey((key) => key + 1)} /></div>
  </main></div>;
}
function SettingsPage({ status, gameState, error, onImported }: { status: AppStatus | null; gameState: GameState; error: boolean; onImported: () => void }) {
  const [importNotice, setImportNotice] = useState<{ text: string; error: boolean } | null>(null);
  const [gameSettings, setGameSettings] = useState<GameDetectionSettings>({ enabled: true });
  const [gameSettingsLoading, setGameSettingsLoading] = useState(true);
  const [gameSettingsSaving, setGameSettingsSaving] = useState(false);
  const [gameNotice, setGameNotice] = useState<{ text: string; error: boolean } | null>(null);
  const [aiSettings, setAiSettings] = useState<AiSettingsView>({ endpoint: "https://api.openai.com/v1", model: "", enabled: false, key_configured: false });
  const [apiKey, setApiKey] = useState("");
  const [aiNotice, setAiNotice] = useState<{ text: string; error: boolean } | null>(null);
  const [savingAi, setSavingAi] = useState(false);
  const [notificationPermission, setNotificationPermission] = useState<"granted" | "denied" | "prompt">("prompt");
  const [notificationNotice, setNotificationNotice] = useState<string | null>(null);

  useEffect(() => {
    const refreshNotificationPermission = () => {
      getNotificationPermission().then(setNotificationPermission).catch(() => setNotificationPermission("denied"));
    };
    getGameDetectionSettings().then(setGameSettings).catch((cause) => setGameNotice({ text: cause instanceof Error ? cause.message : String(cause), error: true })).finally(() => setGameSettingsLoading(false));
    getAiSettings().then(setAiSettings).catch((cause) => setAiNotice({ text: cause instanceof Error ? cause.message : String(cause), error: true }));
    refreshNotificationPermission();
    window.addEventListener("focus", refreshNotificationPermission);
    return () => window.removeEventListener("focus", refreshNotificationPermission);
  }, []);

  async function retryNotificationPermission() {
    const permission = await requestNotificationPermission();
    setNotificationPermission(permission);
    setNotificationNotice(permission === "granted" ? "Windows 通知已允许。" : "通知仍不可用；可在 Windows 设置中检查应用通知权限。");
    if (permission === "granted") void sendAppNotification("海克斯大乱斗小助手", "Windows 通知已启用。");
  }

  async function toggleGameDetection() {
    setGameSettingsSaving(true);
    setGameNotice(null);
    try {
      setGameSettings(await saveGameDetectionSettings(!gameSettings.enabled));
    } catch (cause) {
      setGameNotice({ text: cause instanceof Error ? cause.message : String(cause), error: true });
    } finally {
      setGameSettingsSaving(false);
    }
  }

  async function importFile(file: File | undefined, kind: "rankings" | "augments", input: HTMLInputElement) {
    if (!file) return;
    setImportNotice(null);
    try {
      const contents = await file.text();
      if (kind === "rankings") {
        const result = await importRankingsJson(contents);
        setImportNotice({ text: `已导入 ${result.source || "本地"} · ${result.patch || "未知版本"} 排名数据`, error: false });
      } else {
        const count = await importAugmentGradesJson(contents);
        setImportNotice({ text: `已导入 ${count} 条海克斯分级记录`, error: false });
      }
      onImported();
    } catch (cause) {
      setImportNotice({ text: cause instanceof Error ? cause.message : String(cause), error: true });
    } finally {
      input.value = "";
    }
  }

  async function saveAi() {
    setSavingAi(true);
    setAiNotice(null);
    try {
      const saved = await saveAiSettings({ endpoint: aiSettings.endpoint, model: aiSettings.model, enabled: aiSettings.enabled, api_key: apiKey });
      setAiSettings(saved);
      setApiKey("");
      setAiNotice({ text: saved.key_configured ? "设置已保存；密钥仅保存在 Windows 凭据管理器。" : "设置已保存；本地推荐仍可离线使用。", error: false });
    } catch (cause) {
      setAiNotice({ text: cause instanceof Error ? cause.message : String(cause), error: true });
    } finally {
      setSavingAi(false);
    }
  }

  return <>
    <div className="page-heading"><div><div className="eyebrow">PREFERENCES</div><h1>设置</h1><p>助手在后台托盘运行，游戏内提示会自动显示。</p></div></div>
    <div className="bottom-hint"><Sparkles size={14} /><span>右键系统托盘图标可打开设置或退出；海克斯卡片自动识别无需快捷键。Ctrl+Shift+F6 重识别、Ctrl+Shift+F7 刷新状态、Ctrl+Shift+F8 隐藏浮层（需 Windows 允许注册快捷键）。</span></div>
    <div className="settings-grid">
      <section className="settings-card content-panel"><div className="settings-card-title"><div className="settings-icon blue"><Database size={17} /></div><div><h2>数据状态</h2><p>英雄排名与海克斯推荐分别记录来源</p></div></div><div className="settings-rows"><SettingRow label="英雄总榜" value={error ? "核心暂不可用" : status?.data_status.ready ? `${status.data_status.stale ? "使用缓存" : "已载入"} · ${status.data_status.patch || "补丁未标注"}` : "等待 OP.GG 同步"} /><SettingRow label="海克斯推荐" value={status?.data_status.augment_ready ? "选择英雄后自动更新 · 推荐版本随数据同步" : "选择英雄后同步"} /><SettingRow label="英雄榜来源" value={status?.data_status.source || "OP.GG"} /><SettingRow label="海克斯来源" value="OP.GG（按英雄同步） / 本地缓存" /><SettingRow label="快照生成时间" value={status?.data_status.updated_at || status?.data_status.augment_updated_at || "—"} /></div><div className="import-actions"><label className="import-action"><input type="file" accept=".json,application/json" onChange={(event) => void importFile(event.target.files?.[0], "rankings", event.currentTarget)} /><span><ArrowDownToLine size={14} />导入英雄榜 JSON</span><small>替换当前榜单快照</small></label><label className="import-action"><input type="file" accept=".json,application/json" onChange={(event) => void importFile(event.target.files?.[0], "augments", event.currentTarget)} /><span><ArrowDownToLine size={14} />导入海克斯分级 JSON</span><small>更新 S / A / B 推荐</small></label></div>{importNotice && <div className={`import-notice ${importNotice.error ? "error" : "success"}`} role="status">{importNotice.text}</div>}<div className="settings-tip"><CircleHelp size={14} />启动时同步英雄榜；选择英雄后同步该英雄的海克斯等级，网络异常时继续使用本地缓存。</div></section>
      <section className="settings-card content-panel"><div className="settings-card-title"><div className="settings-icon teal"><Crosshair size={17} /></div><div><h2>游戏客户端</h2><p>选人排名和海克斯提示在游戏内自动显示</p></div></div><div className="connection-state"><span className="connection-state-icon">{gameState.connected ? <Wifi size={18} /> : <WifiOff size={18} />}</span><div><strong>{gameState.connected ? "客户端已连接" : gameSettings.enabled ? "等待客户端连接" : "自动识别已关闭"}</strong><p>{gameState.phase === "champ_select" ? "选人阶段" : gameState.phase === "in_game" ? "对局进行中" : gameState.phase === "loading" ? "正在加载对局" : gameState.player_champion_name || "手动模式可用"}</p></div><span className={`state-dot ${gameState.connected ? "connected" : ""}`} /></div><div className="optional-setting"><div><strong>自动识别当前英雄</strong><p>{gameSettings.enabled ? "已开启 · 读取本机客户端状态" : "已关闭 · 不扫描客户端进程"}</p></div><button className={`toggle ${gameSettings.enabled ? "on" : ""}`} role="switch" aria-checked={gameSettings.enabled} aria-label="自动识别当前英雄" disabled={gameSettingsLoading || gameSettingsSaving} onClick={() => void toggleGameDetection()}><span /></button></div>{gameNotice && <div className={`import-notice ${gameNotice.error ? "error" : "success"}`} role="status">{gameNotice.text}</div>}<div className="optional-setting"><div><strong>Windows 通知</strong><p>{notificationPermission === "granted" ? "已允许 · 启动和连接时提示" : notificationPermission === "denied" ? "未允许 · 游戏浮层仍可正常使用" : "尚未授权 · 首次启动时会自动请求"}</p></div>{notificationPermission !== "granted" && <button className="save-ai-button" onClick={() => void retryNotificationPermission()}>请求权限</button>}</div>{notificationNotice && <div className="import-notice success" role="status">{notificationNotice}</div>}<div className="settings-tip"><Shield size={14} />仅开启后读取英雄联盟客户端本地状态；凭据只用于本机连接，不会上传。</div></section>
      <section className="settings-card settings-card-ai content-panel"><div className="settings-card-title"><div className="settings-icon violet"><Sparkles size={17} /></div><div><h2>AI 与隐私</h2><p>按需启用，分析时才发送请求</p></div></div><div className="ai-fields-grid"><label className="ai-field"><span>兼容接口地址</span><input value={aiSettings.endpoint} onChange={(event) => setAiSettings({ ...aiSettings, endpoint: event.target.value })} placeholder="https://api.openai.com/v1" /></label><label className="ai-field"><span>模型名称</span><input value={aiSettings.model} onChange={(event) => setAiSettings({ ...aiSettings, model: event.target.value })} placeholder="填写服务商提供的模型 ID" /></label><label className="ai-field ai-field-key"><span>API 密钥 {aiSettings.key_configured && <em>已保存</em>}</span><input type="password" autoComplete="new-password" value={apiKey} onChange={(event) => setApiKey(event.target.value)} placeholder={aiSettings.key_configured ? "留空以保留已保存密钥" : "粘贴你的 API 密钥"} /></label></div><div className="optional-setting"><div><strong>分析时调用 AI</strong><p>{aiSettings.enabled ? "已开启 · 点击分析阵容时发送" : "已关闭 · 只使用离线规则"}</p></div><button className={`toggle ${aiSettings.enabled ? "on" : ""}`} role="switch" aria-checked={aiSettings.enabled} aria-label="启用外部 AI 分析" onClick={() => setAiSettings({ ...aiSettings, enabled: !aiSettings.enabled })}><span /></button></div><button className="save-ai-button" onClick={saveAi} disabled={savingAi}><ArrowDownToLine size={13} />{savingAi ? "保存中…" : "保存 AI 设置"}</button>{aiNotice && <div className={`import-notice ${aiNotice.error ? "error" : "success"}`} role="status">{aiNotice.text}</div>}<div className="settings-tip"><Shield size={14} />只有你启用并点击分析时，敌方英雄名称和手动威胁标签才会发往所填服务；不会发送我方阵容、截图或账号信息。</div></section>
    </div>
  </>;
}

function SettingRow({ label, value }: { label: string; value: string }) {
  return <div className="setting-row"><span>{label}</span><strong>{value}</strong></div>;
}

export default App;
