import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { OcrOverlayPayload } from "../../types/domain";
import { findAugmentSynergies, synergySelectionEvent } from "./synergy";

const initialPayload: OcrOverlayPayload = {
  champion_name: null,
  message: "等待识别",
  monitor: null,
  game_window: null,
  cards: [],
};

export default function OcrOverlay() {
  const [payload, setPayload] = useState(initialPayload);
  const [selected, setSelected] = useState<string[]>([]);

  useEffect(() => {
    const subscription = listen<string[]>(synergySelectionEvent, (event) => setSelected(event.payload));
    return () => { void subscription.then((stop) => stop()).catch(console.warn); };
  }, []);

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

  return <OcrCardOverlayView payload={payload} manuallySelected={selected} />;
}

export function OcrCardOverlayView({ payload, manuallySelected = [] }: {
  payload: OcrOverlayPayload;
  manuallySelected?: readonly string[];
}) {
  const selected = [...(payload.selected_augments ?? []), ...manuallySelected];
  return (
    <div className="ocr-card-overlay" aria-live="polite">
      <div className={`ocr-card-canvas${payload.cards.length === 4 && payload.cards.some((card) => card.low_sample) ? " ocr-card-canvas--four-with-warning" : ""}`}>
      {payload.cards.map((card) => {
        const grade = card.matched_name ? card.grade : null;
        const [synergy, ...otherSynergies] = findAugmentSynergies(card.matched_name, selected);
        const rate = card.win_rate;
        const left = card.center_x != null ? `${card.center_x * 100}%` : payload.cards.length === 4
          ? `${18.5 + (card.slot - 1) * 21}%`
          : `${30.8 + (card.slot - 1) * 19.25}%`;
        return <div key={card.slot}>
          <div className={`ocr-card-badge ${grade ? `grade-${grade.toLowerCase()}` : "grade-unknown"}`} style={{ left }} title={card.matched_name || card.raw_text || "未识别"}>
            {grade ?? "?"}
          </div>
          <section className="ocr-card-statistics" style={{ left }} aria-label={`${card.matched_name || "未识别"}推荐`}>
            <strong>{rate != null && Number.isFinite(rate) ? `胜率 ${(rate * 100).toFixed(1)}%` : "暂无可靠胜率"}
              {card.games != null && <span> · {card.games.toLocaleString("zh-CN")} 局</span>}
            </strong>
            {card.low_sample && <span className="sample-warning">小样本</span>}
            {synergy && <p className="ocr-synergy" key={JSON.stringify([card.matched_name, synergy.partners])}>
              <b>{synergy.status === "unverified" ? "联动 · 待实测" : "联动"}{otherSynergies.length > 0 && ` · +${otherSynergies.length}`}</b>
              <span className="ocr-synergy-partners">{synergy.partners.join("＋")}</span>
              <span>{synergy.description}</span>
            </p>}
          </section>
        </div>;
      })}
      {!payload.cards.length && <div className="ocr-card-notice">{payload.message}</div>}
      </div>
    </div>
  );
}
