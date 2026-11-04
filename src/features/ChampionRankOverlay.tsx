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

  const rank = game.ranking;
  return (
    <main className="pool-rank-overlay" aria-live="polite">
      {(game.bench || []).map((entry, index) => entry?.position != null ? <div className="pool-rank-badge" key={`${index}-${entry.champion_id}`} style={{ left: `${27.45 + index * 4.58}%` }} title={rankLabel(entry)}>
        <span className="pool-tier">{entry.badge_tier ?? "—"}</span><span>#{entry.position}</span>
      </div> : null)}
      {rank?.position != null && <div className="selected-rank-badge" title={rankLabel(rank)}><span className="pool-tier">{rank.badge_tier ?? "—"}</span><span>#{rank.position} / {rank.total ?? "—"}</span></div>}
    </main>
  );
}
