import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { GameState } from "../types/domain";

function rankLabel(rank: NonNullable<GameState["ranking"]>) {
  const tier = rank.badge_tier == null ? "" : rank.source?.startsWith("AramHex")
    ? ` · 本应用 ${rank.badge_tier} 档（按胜率名次五等分）`
    : ` · ${rank.badge_tier} 档`;
  return `${rank.name} · ${rank.position}/${rank.total ?? "?"}${tier}`;
}

const EMPTY: GameState = {
  enabled: false,
  connected: false,
  phase: "disconnected",
  source: null,
  player_champion_id: null,
  player_champion_name: null,
  ranking: null,
  team: [],
  bench: [],
};

export default function ChampionRankOverlay() {
  const [game, setGame] = useState<GameState>(EMPTY);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void listen<GameState>("champion-rank:update", (event) => {
      setGame(event.payload);
    }).then((stop) => {
      if (disposed) {
        stop();
        return;
      }
      unlisten = stop;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  return (
    <main className="pool-rank-overlay" aria-live="polite">
      {(game.bench || []).map((entry, index) => entry?.position != null ? <div className="pool-rank-badge" key={`${index}-${entry.champion_id}`} style={{ left: `${27.45 + index * 4.58}%` }} title={rankLabel(entry)}>
        <span className="pool-tier">{entry.badge_tier ?? "—"}</span><span>#{entry.position}</span>
      </div> : null)}
      {(game.team || []).slice(0, 5).map((entry, index) => (
        <div className="selected-rank-badge" key={index} style={{ top: `${24.3 + index * 11.1}%` }} title={entry ? rankLabel(entry) : "暂无英雄数据"}>
          <span className="pool-tier">{entry?.badge_tier ?? "—"}</span>
          <span>{entry?.position != null ? `#${entry.position}` : "暂无排名"}</span>
          <span>{entry?.win_rate != null && Number.isFinite(entry.win_rate) ? `胜率 ${(entry.win_rate * 100).toFixed(1)}%` : "暂无胜率"}</span>
        </div>
      ))}
    </main>
  );
}
