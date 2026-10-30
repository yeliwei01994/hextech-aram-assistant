# 本地数据快照

海克斯大乱斗小助手的英雄榜和海克斯 S/A/B 分级均从本机 SQLite 快照读取。离线核心不绑定任何数据服务，也不会显示胜率。

## 内置海克斯快照

首次启动时，如果本机还没有海克斯分级数据，应用会把仓库内置快照写入 SQLite；已有数据不会被覆盖。快照来自参考项目公开的 [`hero_augments.csv`](https://github.com/Nyx0ra/lol-aram-mayhem-hextech-helper/blob/main/data/hero_augments.csv)，该项目说明其数据抓取自 [OP.GG ARAM: Mayhem](https://op.gg/lol/modes/aram-mayhem)。当前静态文件包含 173 位英雄的海克斯序位，不含英雄总榜，也未声明源数据补丁或准确更新时间。因此应用只提供可搜索英雄列表和海克斯推荐；英雄榜名次与段位徽章继续留空，不会由海克斯序位推算。

应用按每位英雄的海克斯序位标注：1–10 为 S、11–35 为 A、其余为 B。该规则只表示静态文件内的序位分层，不代表胜率或 OP.GG 官方等级。界面所示补丁会标记为「未核实」，快照生成时间是本地转换时间，不代表 OP.GG 的更新时间。

## 手动导入

打开「设置」，分别导入英雄榜 JSON 与海克斯分级 JSON。文件在本机读取，经过格式与字段校验后，以 SQLite 事务整体替换对应快照。验证或写入失败时保留之前的有效快照。

- 英雄榜格式：[rank-snapshot.schema.json](../data/schemas/rank-snapshot.schema.json)
- 海克斯分级格式：[augment-grade-snapshot.schema.json](../data/schemas/augment-grade-snapshot.schema.json)

如需换用新的 `hero_augments.csv`，可运行 `node tools/convert-reference-csv.mjs <CSV路径> <JSON输出路径> [补丁标签]` 将英雄-海克斯序位转换成 S/A/B 快照，再从设置页导入。转换规则默认把每位英雄的前 10 名标为 S、11–35 名标为 A、其余标为 B；这只是标签分层，不会展示或导入胜率。补丁未核实时会明确标记为「未核实」。

英雄榜快照需要声明补丁、来源和更新时间；每名英雄需有唯一 ID、名称、排名和总数，可选提供等级徽章。海克斯快照需为每条记录提供英雄、海克斯名称、S/A/B 等级与适配理由。导入格式拒绝未声明字段，因此不会意外把胜率字段写入数据库。

## 在线来源与自动更新

应用启动时尝试从 OP.GG 同步英雄排行榜，选择英雄后按英雄同步海克斯分级。同步失败时保留本地已有快照；数据版本以实际来源返回的信息为准。OP.GG 的页面结构和使用条款可能变化，使用者应遵守数据来源的相关条款。
