import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  ArrowDownToLine,
  ArrowRight,
  ChevronDown,
  CircleHelp,
  Crosshair,
  Database,
  ExternalLink,
  Gem,
  LayoutDashboard,
  LoaderCircle,
  Search,
  Settings2,
  Shield,
  Sparkles,
  Swords,
  Wifi,
  WifiOff,
  X,
} from "lucide-react";
import { getAiSettings, getAppStatus, getAugmentGrades, getGameDetectionSettings, getGameState, getHeroDetail, hideChampionRankOverlay, hideOcrOverlay, importAugmentGradesJson, importRankingsJson, listHeroRankings, recognizeCurrentScreen, recommendBuild, refreshChampionAugments, saveAiSettings, saveGameDetectionSettings, showChampionRankOverlay, showOcrOverlay } from "./lib/tauri";
import type { AiSettingsView, AppStatus, AugmentGradeRow, BuildAdvice, ChampionDetail, ChampionRankRow, GameDetectionSettings, GameState, RuleTag, TeamBuildInput, ThreatTag } from "./types/domain";

type Page = "rankings" | "builds" | "augments" | "settings";

const navItems: { id: Page; label: string; icon: typeof LayoutDashboard }[] = [
  { id: "rankings", label: "英雄榜", icon: LayoutDashboard },
  { id: "builds", label: "阵容出装", icon: Swords },
  { id: "augments", label: "海克斯推荐", icon: Gem },
  { id: "settings", label: "设置", icon: Settings2 },
];

const threatTags: RuleTag[] = [
  { id: "physical", label: "物理伤害高", hint: "护甲类装备" },
  { id: "magic", label: "魔法伤害高", hint: "魔抗类装备" },
  { id: "healing", label: "回复能力强", hint: "重伤类装备" },
  { id: "shielding", label: "护盾较多", hint: "反护盾装备" },
  { id: "durable", label: "前排较硬", hint: "穿透类装备" },
];

function App() {
  const [page, setPage] = useState<Page>("rankings");
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [gameState, setGameState] = useState<GameState>({ enabled: false, connected: false, phase: "disconnected", source: null, player_champion_id: null, player_champion_name: null, ranking: null, bench: [] });
  const [statusError, setStatusError] = useState(false);
  const [query, setQuery] = useState("");
  const [rankings, setRankings] = useState<ChampionRankRow[]>([]);
  const [selectedChampion, setSelectedChampion] = useState("");
  const [detail, setDetail] = useState<ChampionDetail | null>(null);
  const [augments, setAugments] = useState<AugmentGradeRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshKey, setRefreshKey] = useState(0);
  const ocrBusy = useRef(false);
  const autoDetectionActive = useRef(false);
  const autoOverlayVisible = useRef(false);
  const autoDismissedUntilExit = useRef(false);
  const gameStateRef = useRef(gameState);
  const selectedChampionRef = useRef(selectedChampion);
  const lastDetectedChampionRef = useRef<string | null>(null);
  const refreshedAugments = useRef(new Set<string>());
  gameStateRef.current = gameState;
  selectedChampionRef.current = selectedChampion;
  if (gameState.player_champion_id) lastDetectedChampionRef.current = gameState.player_champion_id;
  else if (gameState.connected && gameState.phase === "disconnected") lastDetectedChampionRef.current = null;

  useEffect(() => {
    getAppStatus().then((nextStatus) => { setStatus(nextStatus); setStatusError(false); }).catch(() => setStatusError(true));
  }, [refreshKey]);

  useEffect(() => {
    let active = true;
    let timer: number | undefined;
    async function refreshGameState() {
      try {
        const nextState = await getGameState();
        if (active) setGameState(nextState);
      } catch {
        if (active) setGameState((current) => ({ ...current, connected: false, phase: "disconnected", source: null, player_champion_id: null, player_champion_name: null, ranking: null }));
      } finally {
        if (active) timer = window.setTimeout(refreshGameState, 1000);
      }
    }
    void refreshGameState();
    return () => { active = false; if (timer) window.clearTimeout(timer); };
  }, []);

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    if (gameState.enabled && gameState.connected && gameState.phase === "champ_select" && (gameState.player_champion_id || gameState.bench.length)) {
      void showChampionRankOverlay(gameState).catch(() => undefined);
    } else {
      void hideChampionRankOverlay().catch(() => undefined);
    }
  }, [gameState]);

  useEffect(() => {
    for (const id of [gameState.player_champion_id, detail?.champion_id]) {
      if (!id || !/^[a-z0-9]+$/i.test(id) || refreshedAugments.current.has(id.toLowerCase())) continue;
      refreshedAugments.current.add(id.toLowerCase());
      void refreshChampionAugments(id).then(() => {
        setRefreshKey((key) => key + 1);
        if (autoDetectionActive.current) autoOverlayVisible.current = false;
      }).catch((error) => {
        refreshedAugments.current.delete(id.toLowerCase());
        console.warn("海克斯同步失败，继续使用本地数据", error);
      });
    }
  }, [gameState.player_champion_id, detail?.champion_id]);

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let disposed = false;
    const unlisten: Array<() => void> = [];
    const register = (name: string, handler: () => void) => {
      void listen(name, handler).then((stop) => {
        if (disposed) stop();
        else unlisten.push(stop);
      });
    };
    const recognize = (automatic: boolean) => {
      if (ocrBusy.current) return;
      if (automatic && autoDismissedUntilExit.current) return;
      ocrBusy.current = true;
      void (async () => {
        const championId = gameStateRef.current.player_champion_id || lastDetectedChampionRef.current || selectedChampionRef.current || null;
        try {
          const payload = await recognizeCurrentScreen(championId, automatic);
          if (!payload) return;
          if (automatic && (!autoDetectionActive.current || autoDismissedUntilExit.current)) return;
          await showOcrOverlay(payload);
          if (automatic) autoOverlayVisible.current = true;
        } catch (cause) {
          if (automatic) return;
          const fallback = {
            champion_name: gameStateRef.current.player_champion_name || null,
            message: cause instanceof Error ? cause.message : String(cause),
            monitor: null,
            game_window: null,
            cards: [],
          };
          await showOcrOverlay(fallback).catch(() => undefined);
        } finally {
          ocrBusy.current = false;
        }
      })();
    };
    register("ocr:request", () => recognize(false));
    register("ocr:auto-request", () => {
      autoDetectionActive.current = true;
      recognize(true);
    });
    register("game:refresh", () => {
      void getGameState().then(setGameState).catch(() => undefined);
    });
    register("rankings:updated", () => {
      setRefreshKey((current) => current + 1);
      void getGameState().then(setGameState).catch(() => undefined);
    });
    register("ocr:hide", () => {
      if (autoDetectionActive.current) autoDismissedUntilExit.current = true;
      autoOverlayVisible.current = false;
      void hideOcrOverlay().catch(() => undefined);
    });
    register("ocr:auto-hide", () => {
      autoDetectionActive.current = false;
      autoOverlayVisible.current = false;
      autoDismissedUntilExit.current = false;
      void hideOcrOverlay().catch(() => undefined);
    });
    return () => {
      disposed = true;
      unlisten.forEach((stop) => stop());
    };
  }, []);

  useEffect(() => {
    function onShortcut(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setPage("rankings");
        window.setTimeout(() => document.getElementById("champion-search")?.focus(), 0);
      }
    }
    window.addEventListener("keydown", onShortcut);
    return () => window.removeEventListener("keydown", onShortcut);
  }, []);

  useEffect(() => {
    let active = true;
    setLoading(true);
    listHeroRankings(query.trim())
      .then((rows) => active && setRankings(rows))
      .catch(() => active && setRankings([]))
      .finally(() => active && setLoading(false));
    return () => { active = false; };
  }, [query, refreshKey]);

  useEffect(() => {
    if (!selectedChampion) {
      setDetail(null);
      setAugments([]);
      return;
    }
    let active = true;
    Promise.all([getHeroDetail(selectedChampion), getAugmentGrades(selectedChampion)])
      .then(([nextDetail, nextAugments]) => {
        if (!active) return;
        setDetail(nextDetail);
        setAugments(nextAugments);
      })
      .catch(() => {
        if (!active) return;
        setDetail(null);
        setAugments([]);
      });
    return () => { active = false; };
  }, [selectedChampion, refreshKey]);

  const selectedName = detail?.name || selectedChampion;

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand-row">
          <div className="brand-mark"><Crosshair size={19} strokeWidth={2.2} /></div>
          <div className="brand-copy"><strong>海克斯大乱斗小助手</strong><span>HEXTECH ARAM ASSISTANT</span></div>
        </div>

        <div className="side-caption">对局工具</div>
        <nav className="main-nav" aria-label="主导航">
          {navItems.map(({ id, label, icon: Icon }) => (
            <button key={id} className={`nav-item ${page === id ? "active" : ""}`} onClick={() => setPage(id)} aria-current={page === id ? "page" : undefined}>
              <Icon size={17} strokeWidth={1.8} aria-hidden="true" />
              <span>{label}</span>
              {id === "builds" && <span className="nav-dot" aria-hidden="true" />}
            </button>
          ))}
        </nav>

        <div className="sidebar-bottom">
          <div className="side-caption">运行状态</div>
          <div className="connection-card">
            <div className={`connection-icon ${gameState.connected ? "connected" : ""}`}>{gameState.connected ? <Wifi size={15} /> : <WifiOff size={15} />}</div>
            <div><strong>{gameState.connected ? "客户端已连接" : gameState.enabled ? "等待客户端" : "手动模式"}</strong><span>{gameState.player_champion_name || (gameState.enabled ? "自动识别已开启" : "自动识别已关闭")}</span></div>
            <span className={`connection-dot ${gameState.connected ? "connected" : ""}`} />
          </div>
          <div className="sidebar-version">本地运行 · v{status?.version ?? "0.1.0"}</div>
        </div>
      </aside>

      <main className="main-area">
        <header className="topbar">
          <div className="breadcrumb"><span>海克斯大乱斗小助手</span><span className="breadcrumb-sep">/</span><strong>{navItems.find((item) => item.id === page)?.label}</strong></div>
          <div className="topbar-actions">
            <div className="offline-pill"><span className="pulse-dot" />离线可用</div>
            <button className="icon-button help-button" aria-label="帮助"><CircleHelp size={17} /></button>
            <div className="profile-chip"><span className="profile-avatar">A</span><span>召唤师</span><ChevronDown size={13} /></div>
          </div>
        </header>

        <div className="page-wrap">
          {page === "rankings" && (
            <RankingsPage
              query={query}
              setQuery={setQuery}
              rankings={rankings}
              loading={loading}
              status={status}
              gameState={gameState}
              statusError={statusError}
              selectedChampion={selectedChampion}
              onSelect={(id) => { setSelectedChampion(id); setPage("augments"); }}
            />
          )}
          {page === "builds" && <BuildPage />}
          {page === "augments" && (
            <AugmentPage champion={selectedChampion} name={selectedName} detail={detail} augments={augments} onPickChampion={setSelectedChampion} />
          )}
          {page === "settings" && <SettingsPage status={status} gameState={gameState} error={statusError} onImported={() => setRefreshKey((key) => key + 1)} />}
        </div>
      </main>
    </div>
  );
}

function PageHeading({ eyebrow, title, description, action }: { eyebrow: string; title: string; description: string; action?: ReactNode }) {
  return <div className="page-heading"><div><div className="eyebrow">{eyebrow}</div><h1>{title}</h1><p>{description}</p></div>{action}</div>;
}

function RankingsPage(props: {
  query: string;
  setQuery: (value: string) => void;
  rankings: ChampionRankRow[];
  loading: boolean;
  status: AppStatus | null;
  gameState: GameState;
  statusError: boolean;
  selectedChampion: string;
  onSelect: (id: string) => void;
}) {
  const data = props.status?.data_status;
  const hasRows = props.rankings.length > 0;
  const hasRankData = Boolean(data?.ready) || props.rankings.some((row) => row.position != null);
  const displayedSource = data?.augment_source || data?.source;
  const displayedUpdate = data?.augment_updated_at || data?.updated_at;
  return (
    <>
      <PageHeading eyebrow="ARAM INTELLIGENCE" title={hasRankData ? "英雄榜" : "英雄列表"} description={hasRankData ? "查找英雄排名，快速进入海克斯推荐与对局准备。" : "等待 OP.GG 排名同步；可搜索英雄查看已载入的海克斯推荐。"} action={<button className="subtle-button" onClick={() => document.getElementById("champion-search")?.focus()}><Search size={15} />搜索英雄</button>} />

      {props.gameState.enabled && props.gameState.connected && props.gameState.phase === "champ_select" && <section className="game-detection-banner content-panel" aria-live="polite">
        <div className="game-banner-icon"><Crosshair size={18} /></div>
        <div className="game-banner-copy"><span>选人阶段 · 本地识别</span><strong>{props.gameState.player_champion_name || "等待选择英雄"}</strong><small>{props.gameState.player_champion_name ? "已识别当前英雄" : "锁定或意向英雄出现后会显示在这里"}</small></div>
        {props.gameState.ranking?.position != null && props.gameState.ranking.total != null ? <div className="game-banner-rank"><span>英雄排名</span><strong>{props.gameState.ranking.position}<i> / {props.gameState.ranking.total}</i></strong></div> : <div className="game-banner-rank"><span>英雄排名</span><strong>暂无数据</strong></div>}
        {props.gameState.ranking?.badge_tier != null && <span className={`tier-badge ${props.gameState.ranking.badge_tier === 1 ? "tier-one" : ""}`}>{props.gameState.ranking.badge_tier}</span>}
        <button className="banner-action" disabled={!props.gameState.player_champion_id} onClick={() => props.onSelect(props.gameState.player_champion_id || "")}>查看海克斯 <ArrowRight size={14} /></button>
      </section>}

      <section className="overview-strip" aria-label="数据概览">
        <div className="overview-cell"><span className="overview-icon violet"><LayoutDashboard size={16} /></span><div><small>英雄总榜</small><strong>{data?.ready ? data.patch || "已载入" : "等待 OP.GG"}</strong></div></div>
        <div className="overview-divider" />
        <div className="overview-cell"><span className="overview-icon cyan"><Database size={16} /></span><div><small>海克斯来源</small><strong>{displayedSource || "本地数据"}</strong></div></div>
        <div className="overview-divider" />
        <div className="overview-cell"><span className="overview-icon amber"><ArrowDownToLine size={16} /></span><div><small>快照生成</small><strong>{displayedUpdate || "等待数据源"}</strong></div></div>
        <span className={`freshness ${data?.stale || data?.augment_stale ? "stale" : data?.ready || data?.augment_ready ? "ready" : ""}`}><span />{data?.stale || data?.augment_stale ? "数据较旧" : data?.ready ? "英雄榜已载入" : data?.augment_ready ? "海克斯推荐可用" : "等待数据"}</span>
      </section>

      {props.statusError && <div className="inline-alert" role="status">本地核心暂不可用。可以先浏览界面，重新打开应用后会自动重试。</div>}

      <section className="content-panel ranking-panel">
        <div className="panel-toolbar">
          <div className="panel-title-group"><h2>{hasRankData ? "英雄排名" : "可查询英雄"}</h2><span className="count-chip">{hasRows ? props.rankings.length : "—"} 位英雄</span></div>
          <label className="search-field" htmlFor="champion-search"><Search size={16} /><input id="champion-search" value={props.query} onChange={(event) => props.setQuery(event.target.value)} placeholder="搜索英雄名称" /><kbd>Ctrl K</kbd></label>
        </div>
        <div className="table-head"><span>{hasRankData ? "排名" : "名次"}</span><span>英雄</span><span>段位</span><span aria-hidden="true" /></div>
        {props.loading ? <div className="loading-state"><LoaderCircle className="spin" size={20} />正在读取本地数据…</div> : hasRows ? (
          <div className="ranking-list">
            {props.rankings.map((row) => <button key={row.champion_id} className={`ranking-row ${props.selectedChampion === row.champion_id ? "selected" : ""}`} onClick={() => props.onSelect(row.champion_id)}>
              <span className="rank-number">{row.position != null && row.total != null ? `${row.position}/${row.total}` : "—"}</span>
              <span className="champion-cell"><span className="champion-portrait">{row.name.slice(0, 1)}</span><strong>{row.name}</strong><span className="champion-id">{row.champion_id}</span></span>
              <span className={`tier-badge ${row.badge_tier === 1 ? "tier-one" : ""}`}>{row.badge_tier ?? "—"}</span>
              <ArrowRight size={15} className="row-arrow" />
            </button>)}
          </div>
        ) : <RankingEmpty query={props.query} />}
        <div className="table-foot"><span>{hasRankData ? <>排名以 <b>1 / 总英雄数</b> 展示 · 不展示胜率</> : "英雄排名与段位数据暂未提供；不会生成虚构名次"}</span><span>等级徽章仅在数据源提供时显示</span></div>
      </section>

      <div className="bottom-hint"><Sparkles size={14} /><span>选中英雄后，可查看海克斯 S / A / B 级推荐与适配理由。</span><button onClick={() => props.onSelect("")}>了解推荐方式 <ArrowRight size={13} /></button></div>
    </>
  );
}

function RankingEmpty({ query }: { query: string }) {
  return <div className="empty-state ranking-empty">
    <div className="empty-art"><div className="empty-orbit orbit-one" /><div className="empty-orbit orbit-two" /><div className="empty-symbol"><Database size={24} strokeWidth={1.5} /></div><span className="orbit-star star-one">✦</span><span className="orbit-star star-two">✧</span></div>
    <h3>{query ? "没有找到匹配的英雄" : "正在等待 OP.GG 英雄榜"}</h3>
    <p>{query ? "检查一下英雄名称，或者清空搜索条件。" : "应用启动时会自动同步 ARAM: Mayhem 排名；网络暂不可用时会保留本地缓存。这里只展示名次与段位，不展示胜率。"}</p>
    {!query && <div className="empty-note"><span className="note-icon"><Shield size={14} /></span><span>不会用虚构排名填充榜单</span></div>}
  </div>;
}

function AugmentPage({ champion, name, detail, augments, onPickChampion }: { champion: string; name: string; detail: ChampionDetail | null; augments: AugmentGradeRow[]; onPickChampion: (value: string) => void }) {
  const grouped = useMemo(() => ({ S: augments.filter((row) => row.grade === "S"), A: augments.filter((row) => row.grade === "A"), B: augments.filter((row) => row.grade === "B") }), [augments]);
  return <>
    <PageHeading eyebrow="CHAMPION GUIDE" title="海克斯推荐" description="按英雄查看适配度分级与简短理由，不用胜率百分比替代推荐结论。" />
    <section className="champion-hero content-panel">
      <div className="champion-hero-avatar">{champion ? name.slice(0, 1) : <Crosshair size={24} />}</div>
      <div className="champion-hero-copy"><span className="section-overline">当前英雄</span><h2>{champion ? name : "选择一位英雄"}</h2><p>{detail?.ranking?.position && detail.ranking.total ? `英雄排名 ${detail.ranking.position} / ${detail.ranking.total}` : "选择英雄后查看对应的海克斯分级"}</p></div>
      {detail?.ranking?.badge_tier && <span className="tier-badge hero-tier">{detail.ranking.badge_tier}</span>}
      <label className="hero-picker"><span>英雄 ID / 名称</span><input value={champion} onChange={(event) => onPickChampion(event.target.value)} placeholder="例如：Teemo" aria-label="输入英雄 ID 或名称" /><X size={14} onClick={() => onPickChampion("")} role="button" aria-label="清空英雄" tabIndex={0} /></label>
    </section>

    <div className="grade-heading"><div><h2>增幅装置分级</h2><p>按 S / A / B 分组 · 仅提供推荐等级与理由</p></div><span className="grade-legend"><i className="grade-dot s" />S 强烈推荐 <i className="grade-dot a" />A 值得选择 <i className="grade-dot b" />B 情况适合</span></div>
    {!champion || augments.length === 0 ? <div className="grade-empty content-panel">
      <div className="grade-empty-icon"><Gem size={22} /></div><div><strong>{champion ? "还没有这位英雄的分级数据" : "还没有可展示的分级数据"}</strong><p>{champion ? `本地数据中暂未收录「${name}」的海克斯推荐。` : "接入经过整理的英雄海克斯分级数据后，会按 S / A / B 自动归类。"}</p></div><span className="grade-empty-status">等待数据集</span>
    </div> : <div className="grade-groups">{(["S", "A", "B"] as const).map((grade) => <GradeGroup key={grade} grade={grade} rows={grouped[grade]} />)}</div>}
    <div className="source-footnote"><Shield size={14} />{!champion ? "选择英雄后会尝试从 OP.GG 更新。" : augments[0]?.reason === "OP.GG 该英雄海克斯等级" ? `OP.GG · ${augments[0].patch || "版本未知"} · 该英雄 S / A / B 等级` : `本地缓存 · ${augments[0]?.patch || "版本未核实"}；选中英雄后会尝试从 OP.GG 更新。`}</div>
  </>;
}

function GradeGroup({ grade, rows }: { grade: "S" | "A" | "B"; rows: AugmentGradeRow[] }) {
  return <section className="grade-group content-panel"><div className="grade-group-head"><span className={`grade-letter grade-${grade.toLowerCase()}`}>{grade}</span><div><h3>{grade === "S" ? "优先推荐" : grade === "A" ? "强力选择" : "情境选择"}</h3><span>{rows.length} 个海克斯</span></div></div>
    {rows.length ? <div className="grade-row-list">{rows.map((row) => <div className="grade-row" key={row.augment_id}><span className="augment-token"><Gem size={15} /></span><div><strong>{row.name}</strong><p>{row.reason}</p></div></div>)}</div> : <div className="grade-group-empty">当前没有 {grade} 级记录</div>}
  </section>;
}

const initialTeam: TeamBuildInput = {
  player_champion_id: "",
  allies: ["", "", "", ""],
  enemies: ["", "", "", "", ""],
  allies_complete: false,
  enemies_complete: false,
  enemy_tags: [],
};

function BuildPage() {
  const [team, setTeam] = useState<TeamBuildInput>(() => {
    try { return { ...initialTeam, ...JSON.parse(localStorage.getItem("aram-team") || "{}") } as TeamBuildInput; }
    catch { return initialTeam; }
  });
  const [advice, setAdvice] = useState<BuildAdvice | null>(null);
  const [pending, setPending] = useState(false);
  const filledEnemies = team.enemies.filter(Boolean).length;
  const filledAllies = team.allies.filter(Boolean).length + (team.player_champion_id ? 1 : 0);

  function updateTeam(next: TeamBuildInput) {
    setTeam(next);
    setAdvice(null);
    localStorage.setItem("aram-team", JSON.stringify(next));
  }

  async function analyze() {
    setPending(true);
    try { setAdvice(await recommendBuild(team)); }
    finally { setPending(false); }
  }

  function setTag(tag: ThreatTag) {
    const tags = team.enemy_tags.includes(tag) ? team.enemy_tags.filter((value) => value !== tag) : [...team.enemy_tags, tag];
    updateTeam({ ...team, enemy_tags: tags });
  }

  return <>
    <PageHeading eyebrow="TEAM COMPOSITION" title="阵容出装" description="离线时手动标注威胁；启用 AI 后，可按敌方英雄名称识别阵容特征。" action={<button className="primary-button" onClick={analyze} disabled={pending}><Sparkles size={15} />{pending ? "分析中…" : "分析阵容"}</button>} />
    <div className="build-layout">
      <div className="build-editor content-panel">
        <div className="editor-top"><div><h2>对局阵容</h2><p>手动录入的信息仅保存在本机</p></div><span className="local-tag"><Shield size={12} />本地</span></div>
        <label className="field-label" htmlFor="player-champion">你的英雄</label>
        <input id="player-champion" className="text-input player-input" value={team.player_champion_id} onChange={(event) => { const player = event.target.value; updateTeam({ ...team, player_champion_id: player, allies_complete: Boolean(player.trim()) && team.allies.every((value) => value.trim()) }); }} placeholder="输入英雄名称或 ID" />
        <div className="team-columns">
          <TeamColumn title="我方队友" caption={`${filledAllies} / 5`} icon="ally" values={team.allies} onChange={(values) => updateTeam({ ...team, allies: values, allies_complete: Boolean(team.player_champion_id.trim()) && values.every((value) => value.trim()) })} />
          <TeamColumn title="敌方阵容" caption={`${filledEnemies} / 5`} icon="enemy" values={team.enemies} onChange={(values) => updateTeam({ ...team, enemies: values, enemies_complete: values.every((value) => value.trim()) })} />
        </div>
        <div className="threat-section"><div className="threat-heading"><strong>敌方阵容特征</strong><span>用于离线规则推断</span></div><div className="threat-chips">{threatTags.map(({ id, label }) => <button key={id} className={`threat-chip ${team.enemy_tags.includes(id) ? "checked" : ""}`} onClick={() => setTag(id)} aria-pressed={team.enemy_tags.includes(id)}><span className="chip-check">{team.enemy_tags.includes(id) ? "✓" : "+"}</span>{label}</button>)}</div></div>
        <div className="editor-footer"><span>输入敌方英雄后，启用 AI 可识别威胁特征；离线时可手动勾选。</span><button onClick={() => { updateTeam(initialTeam); setAdvice(null); }}><X size={13} />清空</button></div>
      </div>
      <BuildAdvicePanel advice={advice} tags={team.enemy_tags} />
    </div>
    <div className="build-note"><CircleHelp size={15} /><span>AI 只会把英雄阵容归纳为五类威胁标签，再交给本地规则生成装备方向；AI 关闭或不可用时，手动标签仍可离线使用。</span></div>
  </>;
}

function TeamColumn({ title, caption, icon, values, onChange }: { title: string; caption: string; icon: "ally" | "enemy"; values: string[]; onChange: (values: string[]) => void }) {
  return <div className="team-column"><div className="team-column-heading"><span className={`team-icon ${icon}`}>{icon === "ally" ? <Shield size={13} /> : <Swords size={13} />}</span><strong>{title}</strong><small>{caption}</small></div>{values.map((value, index) => <div className="team-slot" key={index}><span className="slot-number">{String(index + 1).padStart(2, "0")}</span><input aria-label={`${title} ${index + 1}`} value={value} onChange={(event) => { const next = [...values]; next[index] = event.target.value; onChange(next); }} placeholder="英雄名称" /></div>)}</div>;
}

function BuildAdvicePanel({ advice, tags }: { advice: BuildAdvice | null; tags: ThreatTag[] }) {
  const recommendations = useMemo(() => {
    if (advice) return advice.suggestions;
    return threatTags.filter((tag) => tags.includes(tag.id)).map((tag, index) => ({ item_id: tag.id, item_name: tag.hint, priority: index + 1, reasons: [`已标记敌方特征：${tag.label}`], source: "local_rule" as const }));
  }, [advice, tags]);
  return <section className="advice-panel content-panel">
    <div className="advice-heading"><div className="advice-spark"><Sparkles size={16} /></div><div><h2>装备方向</h2><p>威胁标签由本地规则映射</p></div><span className="offline-small">{advice?.ai_status === "complete" ? "AI 已识别" : "离线规则"}</span></div>
    {advice?.ai_summary && <div className="ai-summary"><strong>阵容分析</strong><p>{advice.ai_summary}</p></div>}
    {advice?.inferred_enemy_tags.length ? <div className="ai-inferred-tags"><span>AI 识别的阵容特征</span><div>{advice.inferred_enemy_tags.map((tag) => <span key={tag}>{threatTags.find((candidate) => candidate.id === tag)?.label || tag}</span>)}</div></div> : null}
    {advice?.ai_status === "unavailable" && <div className="ai-fallback" role="status">AI 暂未返回，下面仍是本地规则建议。</div>}
    {recommendations.length ? <><div className="roster-note">{advice && (!advice.allies_complete || !advice.enemies_complete) ? "阵容尚未填齐，以下是基于已知特征的初步方向。" : "阵容特征已整理，以下建议可按对局进程调整。"}</div><div className="advice-list">{recommendations.map((row, index) => <div className="advice-row" key={row.item_id}><span className="advice-priority">0{index + 1}</span><div><strong>{row.item_name}</strong><p>{row.reasons.join(" · ")}</p></div><span className="priority-tag">{row.source === "ai" ? "AI 判断" : index === 0 ? "优先考虑" : "按局势选择"}</span></div>)}</div><div className="advice-disclaimer">装备方向会随英雄定位与版本数据集进一步细化。</div></> : <div className="advice-empty"><div className="advice-empty-art"><Swords size={21} /></div><strong>等待阵容信息</strong><p>录入敌方英雄，或选择威胁特征<br />就能看到对应的装备方向。</p><div className="mini-pills"><span>护甲</span><span>魔抗</span><span>重伤</span><span>穿透</span></div></div>}
    <div className="advice-foot"><span><span className="foot-check">✓</span>{advice?.ai_status === "complete" ? "AI 已识别" : "离线可用"}</span><span><span className="foot-check">✓</span>说明推荐原因</span></div>
  </section>;
}

function SettingsPage({ status, gameState, error, onImported }: { status: AppStatus | null; gameState: GameState; error: boolean; onImported: () => void }) {
  const [importNotice, setImportNotice] = useState<{ text: string; error: boolean } | null>(null);
  const [gameSettings, setGameSettings] = useState<GameDetectionSettings>({ enabled: false });
  const [gameSettingsLoading, setGameSettingsLoading] = useState(true);
  const [gameSettingsSaving, setGameSettingsSaving] = useState(false);
  const [gameNotice, setGameNotice] = useState<{ text: string; error: boolean } | null>(null);
  const [aiSettings, setAiSettings] = useState<AiSettingsView>({ endpoint: "https://api.openai.com/v1", model: "", enabled: false, key_configured: false });
  const [apiKey, setApiKey] = useState("");
  const [aiNotice, setAiNotice] = useState<{ text: string; error: boolean } | null>(null);
  const [savingAi, setSavingAi] = useState(false);

  useEffect(() => {
    getGameDetectionSettings().then(setGameSettings).catch((cause) => setGameNotice({ text: cause instanceof Error ? cause.message : String(cause), error: true })).finally(() => setGameSettingsLoading(false));
    getAiSettings().then(setAiSettings).catch((cause) => setAiNotice({ text: cause instanceof Error ? cause.message : String(cause), error: true }));
  }, []);

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
    <PageHeading eyebrow="PREFERENCES" title="设置" description="查看本地数据、游戏连接与可选服务状态。" />
    <div className="bottom-hint"><Sparkles size={14} /><span>检测到三张海克斯选择卡片时自动弹出建议；Ctrl+Shift+F6 手动重识别，Ctrl+Shift+F7 刷新英雄，Ctrl+Shift+F8 隐藏浮层。</span></div>
    <div className="settings-grid">
      <section className="settings-card content-panel"><div className="settings-card-title"><div className="settings-icon blue"><Database size={17} /></div><div><h2>数据状态</h2><p>英雄排名与海克斯推荐分别记录来源</p></div></div><div className="settings-rows"><SettingRow label="英雄总榜" value={error ? "核心暂不可用" : status?.data_status.ready ? `${status.data_status.stale ? "使用缓存" : "已载入"} · ${status.data_status.patch || "补丁未标注"}` : "等待 OP.GG 同步"} /><SettingRow label="海克斯推荐" value={status?.data_status.augment_ready ? "选择英雄后自动更新 · 版本见英雄页面" : "选择英雄后同步"} /><SettingRow label="英雄榜来源" value={status?.data_status.source || "OP.GG"} /><SettingRow label="海克斯来源" value="OP.GG（按英雄同步） / 本地缓存" /><SettingRow label="快照生成时间" value={status?.data_status.updated_at || status?.data_status.augment_updated_at || "—"} /></div><div className="import-actions"><label className="import-action"><input type="file" accept=".json,application/json" onChange={(event) => void importFile(event.target.files?.[0], "rankings", event.currentTarget)} /><span><ArrowDownToLine size={14} />导入英雄榜 JSON</span><small>替换当前榜单快照</small></label><label className="import-action"><input type="file" accept=".json,application/json" onChange={(event) => void importFile(event.target.files?.[0], "augments", event.currentTarget)} /><span><ArrowDownToLine size={14} />导入海克斯分级 JSON</span><small>更新 S / A / B 推荐</small></label></div>{importNotice && <div className={`import-notice ${importNotice.error ? "error" : "success"}`} role="status">{importNotice.text}</div>}<div className="settings-tip"><CircleHelp size={14} />启动时同步英雄榜；选择英雄后同步该英雄的海克斯等级，网络异常时继续使用本地缓存。</div></section>
      <section className="settings-card content-panel"><div className="settings-card-title"><div className="settings-icon teal"><Crosshair size={17} /></div><div><h2>游戏客户端</h2><p>断开连接时仍可手动操作</p></div></div><div className="connection-state"><span className="connection-state-icon">{gameState.connected ? <Wifi size={18} /> : <WifiOff size={18} />}</span><div><strong>{gameState.connected ? "客户端已连接" : gameSettings.enabled ? "等待客户端连接" : "自动识别已关闭"}</strong><p>{gameState.phase === "champ_select" ? "选人阶段" : gameState.phase === "in_game" ? "对局进行中" : gameState.phase === "loading" ? "正在加载对局" : gameState.player_champion_name || "手动模式可用"}</p></div><span className={`state-dot ${gameState.connected ? "connected" : ""}`} /></div><div className="optional-setting"><div><strong>自动识别当前英雄</strong><p>{gameSettings.enabled ? "已开启 · 读取本机客户端状态" : "已关闭 · 不扫描客户端进程"}</p></div><button className={`toggle ${gameSettings.enabled ? "on" : ""}`} role="switch" aria-checked={gameSettings.enabled} aria-label="自动识别当前英雄" disabled={gameSettingsLoading || gameSettingsSaving} onClick={() => void toggleGameDetection()}><span /></button></div>{gameNotice && <div className={`import-notice ${gameNotice.error ? "error" : "success"}`} role="status">{gameNotice.text}</div>}<div className="settings-tip"><Shield size={14} />仅启用后读取英雄联盟客户端本地状态；凭据只用于本机连接，不会上传。</div></section>
      <section className="settings-card content-panel"><div className="settings-card-title"><div className="settings-icon violet"><Sparkles size={17} /></div><div><h2>AI 与隐私</h2><p>按需启用，分析时才发送请求</p></div></div><label className="ai-field"><span>兼容接口地址</span><input value={aiSettings.endpoint} onChange={(event) => setAiSettings({ ...aiSettings, endpoint: event.target.value })} placeholder="https://api.openai.com/v1" /></label><label className="ai-field"><span>模型名称</span><input value={aiSettings.model} onChange={(event) => setAiSettings({ ...aiSettings, model: event.target.value })} placeholder="填写服务商提供的模型 ID" /></label><label className="ai-field"><span>API 密钥 {aiSettings.key_configured && <em>已保存</em>}</span><input type="password" autoComplete="new-password" value={apiKey} onChange={(event) => setApiKey(event.target.value)} placeholder={aiSettings.key_configured ? "留空以保留已保存密钥" : "粘贴你的 API 密钥"} /></label><div className="optional-setting"><div><strong>分析时调用 AI</strong><p>{aiSettings.enabled ? "已开启 · 点击分析阵容时发送" : "已关闭 · 只使用离线规则"}</p></div><button className={`toggle ${aiSettings.enabled ? "on" : ""}`} role="switch" aria-checked={aiSettings.enabled} aria-label="启用外部 AI 分析" onClick={() => setAiSettings({ ...aiSettings, enabled: !aiSettings.enabled })}><span /></button></div><button className="save-ai-button" onClick={saveAi} disabled={savingAi}><ArrowDownToLine size={13} />{savingAi ? "保存中…" : "保存 AI 设置"}</button>{aiNotice && <div className={`import-notice ${aiNotice.error ? "error" : "success"}`} role="status">{aiNotice.text}</div>}<div className="settings-tip"><Shield size={14} />只有你启用并点击分析时，敌方英雄名称和手动威胁标签才会发往所填服务；不会发送我方阵容、截图或账号信息。</div></section>
      <section className="settings-card about-card content-panel"><div className="settings-card-title"><div className="settings-icon neutral"><CircleHelp size={17} /></div><div><h2>关于海克斯大乱斗小助手</h2><p>本地优先的 ARAM 对局工具</p></div></div><div className="about-version"><span>当前版本</span><strong>v{status?.version ?? "0.1.0"}</strong></div><a className="source-link" href="https://github.com/Nyx0ra/lol-aram-mayhem-hextech-helper" target="_blank" rel="noreferrer">参考项目 <ExternalLink size={13} /></a></section>
    </div>
  </>;
}

function SettingRow({ label, value }: { label: string; value: string }) {
  return <div className="setting-row"><span>{label}</span><strong>{value}</strong></div>;
}

export default App;
