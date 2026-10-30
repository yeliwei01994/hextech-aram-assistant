import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

const [inputArg, outputArg, patchArg] = process.argv.slice(2);
if (!inputArg || !outputArg) {
  console.error("用法：node tools/convert-reference-csv.mjs <hero_augments.csv> <augment-grades.json> [补丁标签]");
  process.exit(2);
}

const inputPath = resolve(inputArg);
const outputPath = resolve(outputArg);
const raw = await readFile(inputPath, "utf8");
const rows = parseCsv(raw.replace(/^\uFEFF/, ""));
if (rows.length < 2) throw new Error("CSV 文件没有记录");

const headers = rows[0].map((value) => value.trim());
const column = Object.fromEntries(headers.map((name, index) => [name, index]));
const required = ["中文名", "英文名", "总排名", "海克斯名称"];
const missing = required.filter((name) => column[name] === undefined);
if (missing.length) throw new Error(`CSV 缺少列：${missing.join("、")}`);

const records = [];
for (const row of rows.slice(1)) {
  const get = (name) => (row[column[name]] ?? "").trim();
  const championName = get("中文名");
  const championId = get("英文名");
  const augmentName = get("海克斯名称");
  const rank = Number.parseInt(get("总排名"), 10);
  if (!championId || !championName || !augmentName || !Number.isInteger(rank) || rank < 1) continue;
  const grade = rank <= 10 ? "S" : rank <= 35 ? "A" : "B";
  const augmentId = `aug_${createHash("sha256").update(augmentName).digest("hex").slice(0, 16)}`;
  records.push({
    champion_id: championId,
    champion_name: championName,
    augment_id: augmentId,
    name: augmentName,
    grade,
    reason: `该英雄下推荐序位第 ${rank} 名；默认分层为前 10 名 S、前 35 名 A。`,
  });
}
if (!records.length) throw new Error("没有找到可转换的数据行");

const patch = patchArg?.trim() || "未核实";
const snapshot = {
  schema_version: 1,
  patch,
  source: "OP.GG 派生静态数据",
  updated_at: new Date().toISOString(),
  records,
};
await writeFile(outputPath, `${JSON.stringify(snapshot, null, 2)}\n`, "utf8");
console.log(`已转换 ${records.length} 条记录：${outputPath}`);
console.log(`补丁标记：${patch} · 分级：排名 1–10 S、11–35 A、其余 B`);

function parseCsv(text) {
  const result = [];
  let row = [];
  let field = "";
  let quoted = false;
  for (let index = 0; index < text.length; index += 1) {
    const char = text[index];
    if (quoted) {
      if (char === '"' && text[index + 1] === '"') {
        field += '"';
        index += 1;
      } else if (char === '"') {
        quoted = false;
      } else {
        field += char;
      }
    } else if (char === '"' && field.length === 0) {
      quoted = true;
    } else if (char === ",") {
      row.push(field);
      field = "";
    } else if (char === "\n" || char === "\r") {
      if (char === "\r" && text[index + 1] === "\n") index += 1;
      row.push(field);
      if (row.some((value) => value.length > 0)) result.push(row);
      row = [];
      field = "";
    } else {
      field += char;
    }
  }
  if (field.length || row.length) {
    row.push(field);
    if (row.some((value) => value.length > 0)) result.push(row);
  }
  return result;
}
