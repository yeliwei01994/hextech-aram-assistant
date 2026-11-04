import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { OcrOverlayPayload } from "../../types/domain";

const initialPayload: OcrOverlayPayload = {
  champion_name: null,
  message: "等待识别",
  monitor: null,
  game_window: null,
  cards: [],
};

export default function OcrOverlay() {
  const [payload, setPayload] = useState(initialPayload);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let active = true;
    void listen<OcrOverlayPayload>("ocr:results", (event) => setPayload(event.payload)).then((stop) => {
      if (active) unlisten = stop;
      else stop();
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  return (
    <main className="ocr-card-overlay" aria-live="polite">
      <div className="ocr-card-canvas">
      {payload.cards.map((card) => {
        const confirmed = Boolean(card.matched_name && card.grade);
        const left = `${30.8 + (card.slot - 1) * 19.25}%`;
        return <div key={card.slot}>
          <div className={`ocr-card-badge ${confirmed ? `grade-${card.grade?.toLowerCase()}` : "grade-unknown"}`} style={{ left }} title={card.matched_name || card.raw_text || "未识别"}>
            {confirmed ? card.grade : "?"}
          </div>
          <section className="ocr-card-statistics" style={{ left }} aria-label={`${card.matched_name || card.raw_text || "未识别"}推荐`}>
            <strong>{card.win_rate != null ? `胜率 ${(card.win_rate * 100).toFixed(1)}%` : "暂无可靠胜率"}
              {card.games != null && <span> · {card.games.toLocaleString("zh-CN")} 局</span>}
            </strong>
            <p className={card.low_sample ? "sample-warning" : ""}>{card.reason || (card.matched_name ? "暂无可靠数据" : "标题未确认，请重识别")}</p>
          </section>
        </div>;
      })}
      {!payload.cards.length && <div className="ocr-card-notice">{payload.message}</div>}
      {payload.cards.length > 0 && <div className="ocr-data-caption">{payload.champion_name} · {payload.message}</div>}
      </div>
    </main>
  );
}
