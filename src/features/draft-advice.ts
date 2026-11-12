import type { GameState } from "../types/domain";
import heroes from "../../data/seed/aramhex/heroes.json";

// Manual ARAM assessments. Columns: waveclear, poke, engage, peel, frontline, damage.
// Ability: 2 primary, 1 conditional, 0 weak. Damage: P physical, M magic,
// X mixed, B build-dependent. Numeric IDs match the bundled AramHex snapshot.
const rows = `
1 Annie 10200M
2 Olaf 11001P
3 Galio 10122M
4 TwistedFate 21010M
5 XinZhao 00112P
6 Urgot 10111P
7 Leblanc 11000M
8 Vladimir 10001M
9 Fiddlesticks 10210M
10 Kayle 10010B
11 MasterYi 10000P
12 Alistar 00222M
13 Ryze 21010M
14 Sion 20212X
15 Sivir 21000P
16 Soraka 01010M
17 Teemo 11010B
18 Tristana 10010P
19 Warwick 00111X
20 Nunu 10212M
21 MissFortune 21000P
22 Ashe 11210B
23 Tryndamere 10001P
24 Jax 10111X
25 Morgana 11120M
26 Zilean 11120M
27 Singed 20112M
28 Evelynn 00100M
29 Twitch 11000B
30 Karthus 21000M
31 Chogath 11112M
32 Amumu 10212M
33 Rammus 00212M
34 Anivia 21020M
35 Shaco 01010B
36 DrMundo 11002X
37 Sona 00120M
38 Kassadin 10000M
39 Irelia 10101P
40 Janna 00020M
41 Gangplank 21000P
42 Corki 12000P
43 Karma 21010M
44 Taric 00121M
45 Veigar 11120M
48 Trundle 00011P
50 Swain 11112M
51 Caitlyn 21010P
53 Blitzcrank 00210M
54 Malphite 11212B
55 Katarina 10000B
56 Nocturne 10201P
57 Maokai 11212M
58 Renekton 10111P
59 JarvanIV 10211P
60 Elise 01110M
61 Orianna 21110M
62 MonkeyKing 10211P
63 Brand 21000M
64 LeeSin 11111P
67 Vayne 00010P
68 Rumble 21101M
69 Cassiopeia 11010M
72 Skarner 10212X
74 Heimerdinger 21010M
75 Nasus 11011B
76 Nidalee 12000M
77 Udyr 10112B
78 Poppy 11122P
79 Gragas 11121M
80 Pantheon 11110P
81 Ezreal 12000B
82 Mordekaiser 11012M
83 Yorick 11001P
84 Akali 10000M
85 Kennen 11200M
86 Garen 10012P
89 Leona 00212M
90 Malzahar 21010M
91 Talon 10000P
92 Riven 10111P
96 KogMaw 11000B
98 Shen 00122M
99 Lux 22110M
101 Xerath 22000M
102 Shyvana 11101B
103 Ahri 11110M
104 Graves 11001P
105 Fizz 10100M
106 Volibear 10112B
107 Rengar 10101P
110 Varus 12110B
111 Nautilus 00212M
112 Viktor 21010M
113 Sejuani 10212M
114 Fiora 10001P
115 Ziggs 22010M
117 Lulu 01020M
119 Draven 10010P
120 Hecarim 10211P
121 Khazix 11000P
122 Darius 10111P
126 Jayce 12010P
127 Lissandra 10110M
131 Diana 10201M
133 Quinn 11010P
134 Syndra 21120M
136 AurelionSol 21110M
141 Kayn 10101P
142 Zoe 12010M
143 Zyra 21110M
145 Kaisa 11000B
147 Seraphine 11120M
150 Gnar 11211P
154 Zac 10212M
157 Yasuo 10111P
161 Velkoz 12010M
163 Taliyah 21120M
164 Camille 10101P
166 Akshan 11000P
200 Belveth 10101P
201 Braum 00122M
202 Jhin 11110P
203 Kindred 10010P
221 Zeri 11000B
222 Jinx 21010P
223 TahmKench 00122M
233 Briar 10101P
234 Viego 10101P
235 Senna 11010P
236 Lucian 11000P
238 Zed 11000P
240 Kled 10211P
245 Ekko 10101M
246 Qiyana 10200P
254 Vi 10211P
266 Aatrox 10111P
267 Nami 01120M
268 Azir 21120M
350 Yuumi 01010M
360 Samira 10000P
412 Thresh 00121M
420 Illaoi 11001P
421 RekSai 10111P
427 Ivern 00120M
429 Kalista 10110P
432 Bard 01120M
497 Rakan 00221M
498 Xayah 10010P
516 Ornn 10212X
517 Sylas 10111M
518 Neeko 11210M
523 Aphelios 11000P
526 Rell 00212M
555 Pyke 00210P
711 Vex 11210M
777 Yone 10101X
799 Ambessa 10101P
800 Mel 22110M
804 Yunara 11000X
805 Locke 11000B
875 Sett 10212P
876 Lillia 11111M
887 Gwen 10001M
888 Renata 00120M
893 Aurora 11100M
895 Nilah 10111P
897 KSante 10222P
901 Smolder 21000P
902 Milio 01020M
904 Zaahen 10111P
910 Hwei 22020M
950 Naafiri 11000P
`;

type Ability = "waveclear" | "poke" | "engage" | "peel" | "frontline";
type Level = "primary" | "conditional" | "weak" | "unknown";
type Damage = "physical" | "magic" | "mixed" | "build-dependent" | "unknown";
type Profile = { levels: Record<Ability, Level>; damage: Damage };

const abilityOrder: Ability[] = ["waveclear", "poke", "engage", "peel", "frontline"];
const levelsByCode: Record<string, Level> = { 0: "weak", 1: "conditional", 2: "primary" };
const damageByCode: Record<string, Damage> = { P: "physical", M: "magic", X: "mixed", B: "build-dependent" };
const profiles = new Map<string, Profile>();
const names = heroes as Record<string, { name: string; title: string }>;

function normalize(value: string) {
  return value.toLowerCase().replace(/[^\p{L}\p{N}]/gu, "");
}

for (const line of rows.trim().split("\n")) {
  const [id, champion, codes] = line.trim().split(/\s+/);
  const levels = Object.fromEntries(abilityOrder.map((ability, index) =>
    [ability, levelsByCode[codes[index]]],
  )) as Record<Ability, Level>;
  const entry = { levels, damage: damageByCode[codes[5]] };
  const name = names[id];
  for (const alias of [id, champion, name?.name, name?.title]) {
    if (alias) profiles.set(normalize(alias), entry);
  }
}

for (const [alias, id] of Object.entries({ wukong: "62", asol: "136", nunuandwillump: "20" })) {
  const entry = profiles.get(id);
  if (entry) profiles.set(alias, entry);
}

function profile(id: string): Profile {
  return profiles.get(normalize(id)) ?? {
    levels: { waveclear: "unknown", poke: "unknown", engage: "unknown", peel: "unknown", frontline: "unknown" },
    damage: "unknown",
  };
}

export function draftLabels(id: string): string[] {
  const { levels, damage } = profile(id.toLowerCase());
  const labels = dimensions.filter(({ ability }) =>
    levels[ability] === "primary" || levels[ability] === "conditional",
  ).map(({ ability, label }) => levels[ability] === "conditional" ? `${label}（条件）` : label);
  const damageLabel = {
    physical: "物理", magic: "魔法", mixed: "混合",
    "build-dependent": "伤害随出装", unknown: "",
  }[damage];
  if (damageLabel) labels.push(damageLabel);
  return labels;
}

const dimensions: { ability: Ability; target: number; label: string; weight: number }[] = [
  { ability: "waveclear", target: 2, label: "清线", weight: 3 },
  { ability: "poke", target: 3, label: "远程消耗", weight: 2 },
  { ability: "engage", target: 1, label: "开团", weight: 2 },
  { ability: "peel", target: 1, label: "反手保护", weight: 2 },
  { ability: "frontline", target: 1, label: "前排承伤", weight: 2 },
];

function bounds(levels: Level[]) {
  const known = levels.reduce((sum, level) =>
    sum + (level === "primary" ? 1 : level === "conditional" ? 0.5 : 0), 0);
  return { min: known, max: known + levels.filter((level) => level === "unknown").length };
}

export function recommendDraftSwap(game: GameState) {
  if (game.phase !== "champ_select" || game.team.length !== 5) return null;
  const current = game.player_champion_id?.toLowerCase();
  const ids = game.team.map((entry) => entry?.champion_id.toLowerCase() ?? "");
  if (!current || !ids.includes(current) || ids.indexOf(current) !== ids.lastIndexOf(current)) return null;

  const teammates = ids.filter((id) => id !== current).map(profile);
  const original = profile(current);
  const candidates = game.bench.flatMap((entry) => {
    const id = entry?.champion_id.toLowerCase();
    if (!entry || !id || ids.includes(id)) return [];
    const replacement = profile(id);
    const reasons: string[] = [];
    const retained: string[] = [];
    let score = 0;
    const poke = bounds([...teammates.map((hero) => hero.levels.poke), original.levels.poke]);

    for (const { ability, target, label, weight } of dimensions) {
      const others = teammates.map((hero) => hero.levels[ability]);
      const before = bounds([...others, original.levels[ability]]);
      const after = bounds([...others, replacement.levels[ability]]);
      // A shared unknown teammate cancels out when comparing the two candidates.
      const oldValue = bounds([original.levels[ability]]);
      const newValue = bounds([replacement.levels[ability]]);
      const safeMinimum = ability === "waveclear" ? 1 : target;
      if (oldValue.min > newValue.max && before.min >= safeMinimum && after.min < safeMinimum) return [];
      if (after.min >= target && before.min >= target) retained.push(label);
      if (before.max >= target || newValue.min <= oldValue.max) continue;
      if (ability === "engage" && poke.min >= 3) continue;
      reasons.push(`补${label}`);
      score += ability === "peel" && poke.min >= 3 ? 4 : weight;
    }

    // Count dedicated damage sources. Mixed or build-dependent heroes do not
    // guarantee either damage type at champion select.
    for (const type of ["physical", "magic"] as const) {
      const hasSource = teammates.some((hero) => hero.damage === type);
      const uncertain = teammates.some((hero) =>
        hero.damage === "unknown" || hero.damage === "build-dependent" || hero.damage === "mixed");
      if (!hasSource && original.damage === type && replacement.damage !== type
        && replacement.damage !== "unknown") return [];
      if (!hasSource && !uncertain && original.damage !== type && original.damage !== "unknown"
        && original.damage !== "build-dependent" && original.damage !== "mixed" && replacement.damage === type) {
        reasons.push(type === "physical" ? "补物理输出" : "补魔法输出");
        score += 2;
      }
    }

    if (!score) return [];
    if (retained.includes("清线") && !reasons.includes("补清线")) reasons.push("保留清线");
    else if (retained.includes("远程消耗") && !reasons.includes("补远程消耗")) reasons.push("保留消耗");
    return [{ entry, score, reasons: reasons.slice(0, 3) }];
  });

  return candidates.sort((a, b) => b.score - a.score
    || (a.entry.position ?? Infinity) - (b.entry.position ?? Infinity))[0] ?? null;
}
