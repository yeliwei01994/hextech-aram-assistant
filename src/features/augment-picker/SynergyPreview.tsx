import { useState } from "react";
import type { OcrOverlayPayload } from "../../types/domain";
import { OcrCardOverlayView } from "./OcrOverlay";

const scenarios = [
  { name: "双卡联动", rounds: [["虚幻武器", "易损", "冰寒"], ["秘术冲拳", "会心治疗", "珠光护手"]] },
  { name: "待实测联动", rounds: [["小蜜蜂！！！", "易损", "冰寒"], ["秘术冲拳", "会心治疗", "珠光护手"]] },
  { name: "四卡 · 三卡联动", rounds: [
    ["珠光护手", "冰寒", "史上最大雪球", "终极刷新"],
    ["易损", "火上浇油", "灵魄炸弹", "秘术冲拳"],
    ["大腕", "暴击律动", "会心治疗", "溢流"],
  ] },
] as const;

export default function SynergyPreview() {
  const [scenarioIndex, setScenarioIndex] = useState(0);
  const [selected, setSelected] = useState<string[]>([]);
  const scenario = scenarios[scenarioIndex];
  const round = Math.min(selected.length, scenario.rounds.length - 1);
  const complete = selected.length === scenario.rounds.length;
  const names: readonly string[] = scenario.rounds[round];
  const payload: OcrOverlayPayload = {
    champion_name: null,
    message: "",
    monitor: null,
    game_window: null,
    selected_augments: selected.slice(0, round),
    cards: names.map((name, index) => ({
      slot: index + 1, raw_text: name, confidence: 1, matched_name: name,
      candidate_name: name, grade: "B", reason: null, win_rate: 0.464, games: 5784,
    })),
  };

  return <main className="synergy-preview">
    <h1>海克斯联动预览</h1>
    <p>点击卡片进入下一轮。胜率与局数为演示数据。</p>
    <div className="synergy-preview-controls">
      <label>场景 <select value={scenarioIndex} onChange={(event) => {
        setScenarioIndex(Number(event.target.value));
        setSelected([]);
      }}>
        {scenarios.map(({ name }, index) => <option key={name} value={index}>{name}</option>)}
      </select></label>
      <button type="button" onClick={() => setSelected([])}>重新开始</button>
    </div>
    <p role="status">第 {round + 1} 轮 · 已选 {selected.join("、") || "暂无"}{complete && " · 模拟结束"}</p>
    <div className="synergy-preview-stage">
      <OcrCardOverlayView payload={payload} />
      {names.map((name, index) => {
        const center = names.length === 4 ? 18.5 + index * 21 : 30.8 + index * 19.25;
        return <button key={name} type="button" className="synergy-preview-card"
          style={{ left: `${center}%` }} disabled={complete}
          onClick={() => setSelected((previous) => [...previous, name])}>{name}</button>;
      })}
    </div>
  </main>;
}
