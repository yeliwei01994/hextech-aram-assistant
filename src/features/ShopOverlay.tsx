import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getGameState } from "../lib/tauri";

interface EquipmentStats {
  champion: string;
  patch: string;
  route: { patch: string; core: string[][] } | null;
  starters: { id: string; name: string; price: number }[][];
  items: {
    id: string; name: string; games: number; winRate: number; price: number | null;
    components: string[] | null;
    componentOptions: { name: string; price: number }[];
  }[];
}

interface ShopContext {
  champion: string;
  enemies: { id: string; name: string | null }[];
  owned: string[] | null;
  gold: number | null;
  gameTime: number | null;
}

const counters = [
  { heroes: ["soraka", "sona", "nami", "yuumi", "aatrox", "drmundo", "vladimir", "warwick", "众星之子", "索拉卡"],
    items: ["3011", "3033", "3075", "3165", "6609"], reason: "可考虑重伤" },
  { heroes: ["karma", "lulu", "janna", "milio", "ivern", "seraphine", "卡尔玛", "璐璐"],
    items: ["6695"], reason: "可考虑破盾" },
  { heroes: ["ornn", "sion", "chogath", "tahmkench", "drmundo", "奥恩", "赛恩"],
    items: ["3036", "3135", "3137"], reason: "可考虑穿透" },
];

type Equipment = EquipmentStats["items"][number];

function purchaseNote(item: Equipment, context: ShopContext | null) {
  if (item.components?.some((id) => context?.owned?.includes(id))) return "可接已有散件 · 合成价以商店为准";
  if (item.price == null || context?.gold == null || context.owned == null) return null;
  if (context.gold >= item.price) return "金币足够支付标价";
  return `距成装标价还差 ${Math.ceil(item.price - context.gold)} 金`;
}

function EquipmentRow({ item, context }: { item: Equipment; context: ShopContext | null }) {
  return <li>
    <strong>{item.name}</strong>
    <span>持有胜率 {(item.winRate * 100).toFixed(1)}% · {item.games.toLocaleString("zh-CN")} 局</span>
    <span>{purchaseNote(item, context)}</span>
  </li>;
}

export default function ShopOverlay() {
  const [open, setOpen] = useState(false);
  const [stats, setStats] = useState<EquipmentStats | null>(null);
  const [context, setContext] = useState<ShopContext | null>(null);
  useEffect(() => {
    let disposed = false;
    const listener = listen<boolean>("shop:visibility", ({ payload }) => {
      if (!disposed) setOpen(payload);
    });
    return () => {
      disposed = true;
      void listener.then((stop) => stop()).catch(console.warn);
    };
  }, []);

  useEffect(() => {
    setStats(null);
    setContext(null);
    if (!open) return;
    let disposed = false;
    let retry: number | undefined;
    let loadedChampion: string | undefined;
    async function load() {
      try {
        const live = await invoke<ShopContext | null>("get_live_shop_context");
        if (disposed) return;
        if (live) {
          setContext((previous) => previous?.champion === live.champion ? {
            ...live,
            owned: live.owned ?? previous.owned,
            gold: live.gold ?? previous.gold,
            enemies: live.enemies.length ? live.enemies : previous.enemies,
          } : live);
        }
        let champion = live?.champion;
        if (!champion && !loadedChampion) {
          const game = await getGameState();
          if (!game.enabled) return;
          if (game.phase === "in_game") champion = game.player_champion_id ?? undefined;
        }
        if (disposed) return;
        if (champion && champion !== loadedChampion) {
          const result = await invoke<EquipmentStats | null>("get_equipment_stats", {
            champion,
          });
          if (disposed) return;
          if (result?.items.length) {
            setStats(result);
            loadedChampion = champion;
          }
        }
      } catch (error) {
        console.warn("读取装备建议失败", error);
      }
      if (!disposed) retry = window.setTimeout(load, 1500);
    }
    void load();
    return () => { disposed = true; window.clearTimeout(retry); };
  }, [open]);

  if (!open) return null;
  if (!stats) return <aside className="shop-advice" aria-label="装备建议">
    <strong>装备建议</strong>
    <small>等待识别当前英雄或同步装备数据</small>
  </aside>;
  const available = stats.items.filter((item) => !context?.owned?.includes(item.id));
  const common = available.slice(0, 8).filter((item) => item.games >= (available[0]?.games ?? 0) * 0.2);
  const matches = (item: Equipment) => new Set(
    item.components?.filter((id) => context?.owned?.includes(id)),
  ).size;
  const sorted = [...common].sort((a, b) => matches(b) - matches(a) || b.games - a.games);
  const targeted = counters.flatMap((rule) => {
    const enemy = context?.enemies.find(({ id, name }) => rule.heroes.includes(id.toLowerCase()) || (name && rule.heroes.includes(name)));
    const item = available.find(({ id, games }) =>
      rule.items.includes(id) && games >= (stats.items[0]?.games ?? 0) * 0.02,
    );
    return enemy && item ? [{ item, reason: `敌方有 ${enemy.name ?? enemy.id}，${rule.reason}` }] : [];
  })[0];
  const route = stats.route;
  const knownItems = new Set(stats.items.map(({ id }) => id));
  const owned = new Set(context?.owned ?? []);
  const groups = (route?.core ?? [])
    .map((group) => group.filter((id) => knownItems.has(id)))
    .filter((group) => group.length >= 2);
  const overlap = (group: string[]) => group.filter((id) => owned.has(id)).length;
  const group = groups.sort((a, b) => overlap(b) - overlap(a))[0];
  const core = available.find((item) => item.id === group?.find((id) => !owned.has(id)));
  const next = core ?? sorted.find((item) => item.id !== targeted?.item.id);
  // Utility slots can be populated before the first equipment purchase.
  const utilityItems = new Set(["2003", "2031", "2052", "3340", "3363", "3364"]);
  const starterIds = new Set(stats.starters.flat().map(({ id }) => id));
  const hasGear = context?.owned?.some((id) => knownItems.has(id)
    || stats.items.some((item) => item.components?.includes(id))
    || starterIds.has(id) && !utilityItems.has(id));
  const opening = context?.owned != null && !hasGear
    && (context.gameTime != null ? context.gameTime <= 105 : (context.gold ?? 0) <= 1700);
  const inventoryPending = context?.owned == null;
  const starter = opening && route ? stats.starters.map((items) => {
    const inventory = [...(context.owned ?? [])];
    return items.filter(({ id }) => {
      const index = inventory.indexOf(id);
      if (index < 0) return true;
      inventory.splice(index, 1);
      return false;
    });
  }).find((items) =>
    items.length > 0 && (context.gold == null
      || items.reduce((sum, item) => sum + item.price, 0) <= context.gold),
  ) : undefined;
  const openingRoute = opening && !starter && context.gold != null ? common.flatMap((item) => {
    if (item.price == null || item.price <= context.gold!) return [];
    const component = item.componentOptions
      .filter(({ price }) => price <= context.gold!)
      .sort((a, b) => b.price - a.price)[0];
    return component ? [{ item, component }] : [];
  })[0] : undefined;
  return <aside className="shop-advice" aria-label="装备建议">
    <strong>{stats.champion} · 装备建议</strong>
    <small>国服 {stats.patch} · AramHex · CC BY-NC 4.0</small>
    {inventoryPending && <small>等待读取背包，确认出装阶段</small>}
    {starter && <><small>OP.GG {route?.patch} 出门装参考</small><ul><li>
      <strong>{starter.map(({ name }) => name).join(" + ")}</strong>
      <span>{starter.reduce((sum, item) => sum + item.price, 0)} 金 · 标价以商店为准</span>
    </li></ul></>}
    {openingRoute && <><small>常见成装的散件参考</small><ul><li>
      <strong>{openingRoute.component.name} · {openingRoute.component.price} 金</strong>
      <span>可合成 {openingRoute.item.name} · 标价以商店为准</span>
    </li></ul></>}
    {opening && !starter && !openingRoute && <small>暂无符合当前金币的散件建议</small>}
    {!opening && !inventoryPending && next && <><small>{core ? `OP.GG ${route?.patch} 路线参考` : "常见成装参考"}</small>
      <ul><EquipmentRow item={next} context={context} /></ul></>}
    {!opening && !inventoryPending && targeted && targeted.item.id !== next?.id && <><small>{targeted.reason}</small>
      <ul><EquipmentRow item={targeted.item} context={context} /></ul></>}
    {!opening && !inventoryPending && !next && !targeted && <small>暂无可推荐的成装</small>}
  </aside>;
}
