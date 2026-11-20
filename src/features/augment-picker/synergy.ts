type SynergyStatus = "card-supported" | "unverified";

const rules: ReadonlyArray<readonly [cards: readonly string[], status: SynergyStatus, description: string]> = [
  [["虚幻武器","秘术冲拳"],"card-supported","技能附带的攻击特效可触发减冷却。"],
  [["小蜜蜂！！！","秘术冲拳"],"unverified","蜂群可能额外触发减冷却。"],
  [["虚幻武器","伤不过三"],"unverified","技能命中能否推进三击计数。"],
  [["小蜜蜂！！！","伤不过三"],"unverified","蜂群能否叠三击真伤。"],
  [["虚幻武器","小蜜蜂！！！"],"card-supported","两张都能提供攻击特效来源。"],
  [["小蜜蜂！！！","物法皆修"],"unverified","蜂群能否叠加物法增益。"],
  [["虚幻武器","吸元秘术"],"unverified","技能能否消耗充能并回血。"],
  [["小蜜蜂！！！","吸元秘术"],"unverified","蜂群能否触发充能伤害和治疗。"],
  [["虚幻武器","可靠武器"],"unverified","技能能否推进武器计数。"],
  [["小蜜蜂！！！","可靠武器"],"unverified","蜂群能否推进武器计数。"],
  [["珠光护手","珠光手镯"],"card-supported","符合条件的大招可分别判定两次暴击。"],
  [["珠光护手","大腕"],"unverified","技能暴击能否吃到额外暴伤。"],
  [["易损","大腕"],"unverified","持续伤害和装备暴击能否吃到额外暴伤。"],
  [["珠光护手","易损"],"card-supported","拓宽可暴击的伤害来源。"],
  [["虚幻武器","鼓声暴起 低音轰鸣"],"unverified","技能能否推进鼓声触发。"],
  [["炼狱导管","火上浇油"],"card-supported","普攻叠灼烧，跳伤返还小技能冷却。"],
  [["炼狱导管","易损"],"unverified","导管灼烧能否暴击。"],
  [["虚幻武器","火上浇油"],"unverified","技能附带的攻击特效能否叠灼烧。"],
  [["弹跳蜜蜂","秘术冲拳"],"unverified","一次弹射能否多次退还冷却。"],
  [["弹跳蜜蜂","火上浇油"],"unverified","弹射能否给多个目标挂灼烧。"],
  [["弹跳蜜蜂","三重射击"],"unverified","额外弹道能否分别弹射。"],
  [["死亡之环","吸元秘术"],"unverified","充能回血能否转为环伤。"],
  [["死亡之环","海洋龙魂"],"unverified","龙魂回血能否持续转为环伤。"],
  [["死亡之环","灵魄炸弹"],"unverified","群体治疗能否转为多次环伤。"],
  [["死亡之环","会心治疗"],"unverified","治疗暴击能否放大环伤。"],
  [["灵魄炸弹","风语者的祝福"],"unverified","炸弹能否给多名队友附加双抗。"],
  [["风语者的祝福","蛋白粉奶昔"],"unverified","临时双抗能否强化下一次治疗或护盾。"],
  [["珠光护手","会心治疗"],"card-supported","技能伤害和治疗护盾共用暴击率。"],
  [["珠光护手","暴击律动"],"unverified","技能暴击能否叠攻速。"],
  [["易损","暴击律动"],"unverified","持续伤害或装备暴击能否叠攻速。"],
  [["大腕","暴击律动"],"unverified","临时攻速能否实时转成暴伤。"],
  [["由心及物","翻倍时间"],"unverified","连招叠出的法力能否实时转生命。"],
  [["由心及物","歌利亚巨人"],"unverified","法力转化的生命能否再次放大。"],
  [["溢流","翻倍时间"],"card-supported","连招永久叠法力，强化溢流增益。"],
  [["终极刷新","终极唤醒"],"card-supported","连续开大，注意小技能重置的冷却。"],
  [["终极唤醒","你摸不到"],"card-supported","开大进入免疫窗口并重置小技能。"],
  [["终极刷新","你摸不到"],"unverified","第二次开大能否再次免疫。"],
  [["弹球","史上最大雪球"],"unverified","反弹、变大和击飞能否同时保留。"],
  [["急急小子","循环往复"],"card-supported","固定技能急速可转为额外移速。"],
  [["急急小子","无限循环往复"],"card-supported","基础急速和击杀叠出的急速都可转为移速。"],
  [["急急小子","尤里卡"],"card-supported","法强先转为技能急速，再转为移速。"],
  [["急急小子","缩小引擎"],"card-supported","击杀叠出的技能急速还能转为额外移速。"],
  [["急急小子","科学狂人"],"card-supported","缩小形态提供的技能急速可额外转成移速。"],
  [["急急小子","纯粹主义者 - 术师"],"card-supported","攻速转为技能急速，再转成移速。"],
  [["急急小子","面包和黄油"],"unverified","专属急速能否额外转为移速。"],
  [["急急小子","面包和果酱"],"unverified","专属急速能否额外转为移速。"],
  [["急急小子","面包和奶酪"],"unverified","专属急速能否额外转为移速。"],
  [["急急小子","全凭身法"],"unverified","专属急速能否额外转为移速。"],
  [["急急小子","终极唤醒"],"unverified","专属急速能否额外转为移速。"],
  [["急急小子","终极循环往复"],"unverified","专属急速能否额外转为移速。"],
  [["急急小子","秘术冲拳"],"card-supported","走位拉扯，攻击特效返还技能冷却。"],
  [["急急小子","炼狱导管"],"card-supported","走位拉扯，灼烧持续返还小技能冷却。"],
  [["急急小子","老练狙神"],"card-supported","用移速拉开距离，远程技能命中时返还冷却。"],
  [["急急小子","大法师"],"card-supported","移速帮助走位，交替施法返还技能冷却。"],
  [["急急小子","超负荷"],"card-supported","用移速调整站位，交替施法刷新强化技能。"],
  [["急急小子","深红钴蓝闪光"],"card-supported","用移速接近标记目标，普攻返还对应技能冷却。"],
  [["急急小子","终极刷新"],"card-supported","用移速调整进场位置，利用大招重置打第二轮。"],
  [["急急小子","奥术公理"],"card-supported","游走收割，参与击杀返还大招冷却。"],
  [["急急小子","回归基本功"],"card-supported","额外技能急速可转为移速，代价是无法使用大招。"],
  [["急急小子","急速之追求"],"unverified","任务奖励的单技能急速能否转为移速。"],
  [["急急小子","重型"],"unverified","强化技能的专属急速能否转为移速。"],
  [["急急小子","力量"],"unverified","强化技能的专属急速能否转为移速。"],
  [["急急小子","鞭挞"],"unverified","强化技能的专属急速能否转为移速。"],
  [["急急小子","厄运"],"unverified","强化技能的专属急速能否转为移速。"],
  [["急急小子","单曲红人"],"card-supported","通用急速同时提供移速并提高强化技能伤害。"],
  [["尤里卡","超凡邪恶"],"card-supported","叠出的永久法强可继续转成技能急速。"],
  [["尤里卡","巫师式思考"],"card-supported","额外法强同时提高技能急速。"],
  [["纯粹主义者 - 术师","灵巧"],"card-supported","额外攻击速度转成技能急速。"],
  [["珠光护手","易损","大腕"],"unverified","三类伤害的暴击能否吃到额外暴伤。"],
  [["火上浇油","炼狱导管","易损"],"unverified","灼烧暴击与小技能冷却返还。"],
  [["会心治疗","灵魄炸弹","死亡之环"],"unverified","群体暴击治疗能否转为多次环伤。"],
  [["由心及物","翻倍时间","溢流"],"unverified","连招叠法力，同时换取生命和溢流增益。"],
  [["终极刷新","终极唤醒","你摸不到"],"unverified","连续开大能否再次重置小技能并免疫。"],
];

export const synergyCards = [...new Set(rules.flatMap(([cards]) => cards))];
export const synergySelectionEvent = "ocr:synergy-selection";

function normalizeName(name: string) {
  return name.normalize("NFKC").replace(/[\s\p{P}]/gu, "");
}

export function findAugmentSynergies(candidate: string | null | undefined, selected: readonly string[]) {
  if (!candidate) return [];

  const candidateName = normalizeName(candidate);
  const owned = new Set(selected.map(normalizeName));
  if (owned.has(candidateName)) return [];

  return rules.flatMap(([cards, status, description]) => {
    if (!cards.some((name) => normalizeName(name) === candidateName)) return [];

    const partners = cards.filter((name) => normalizeName(name) !== candidateName);
    if (!partners.every((name) => owned.has(normalizeName(name)))) return [];

    return [{ partners, status, description }];
  }).sort((first, second) =>
    Number(first.status === "unverified") - Number(second.status === "unverified") ||
    second.partners.length - first.partners.length);
}
