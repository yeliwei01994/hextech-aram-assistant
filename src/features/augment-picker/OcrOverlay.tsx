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
        return <div key={card.slot} className={`ocr-card-badge ${confirmed ? `grade-${card.grade?.toLowerCase()}` : "grade-unknown"}`} style={{ left: `${30.8 + (card.slot - 1) * 19.25}%` }} title={card.matched_name || card.raw_text || "未识别"}>
          {confirmed ? card.grade : "?"}
        </div>;
      })}
      {!payload.cards.length && <div className="ocr-card-notice">{payload.message}</div>}
      </div>
    </main>
  );
}
