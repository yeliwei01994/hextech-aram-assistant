//! Public aggregate statistics, attributed under CC BY-NC 4.0.

use crate::AppState;
use reqwest::Client;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};

const SOURCE: &str = "AramHex · 国服全体样本 · CC BY-NC 4.0";
const BASE: &str = "https://raw.githubusercontent.com/czgreat/aramhex-data";
const FILES: [&str; 4] = [
    "heroes.json",
    "hero_augments.json",
    "augments.json",
    "meta.json",
];
const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Deserialize, Serialize)]
struct FileInfo {
    sha256: String,
    size: usize,
}

#[derive(Deserialize, Serialize)]
struct Manifest {
    schema_version: u16,
    seq: u64,
    patch: String,
    path: String,
    mode: String,
    snapshot_at: String,
    files: BTreeMap<String, FileInfo>,
}

impl Manifest {
    fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1
            || self.seq == 0
            || self.mode != "normal"
            || self.patch.is_empty()
            || self.patch.len() > 24
            || !self.patch.chars().all(|c| c.is_ascii_digit() || c == '.')
            || self.path != format!("patches/{}/normal", self.patch)
            || chrono::DateTime::parse_from_rfc3339(&self.snapshot_at).is_err()
        {
            return Err("AramHex 更新清单版本、规则或路径无效".into());
        }
        for name in FILES {
            let f = self
                .files
                .get(name)
                .ok_or_else(|| format!("AramHex 缺少 {name}"))?;
            if f.size == 0
                || f.size > MAX_FILE_BYTES
                || f.sha256.len() != 64
                || !f.sha256.chars().all(|c| c.is_ascii_hexdigit())
            {
                return Err(format!("AramHex 文件信息无效：{name}"));
            }
        }
        Ok(())
    }

    fn verify(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let f = self.files.get(name).ok_or("更新清单缺少文件")?;
        if bytes.len() != f.size
            || format!("{:x}", Sha256::digest(bytes)) != f.sha256.to_ascii_lowercase()
        {
            return Err(format!("AramHex 文件校验失败：{name}，保留本地缓存"));
        }
        Ok(())
    }

    fn identity(&self) -> String {
        serde_json::to_string(self).expect("manifest serializes")
    }

    fn needs_update(&self, db: &Connection) -> Result<bool, String> {
        let old: Option<String> = db
            .query_row(
                "SELECT value FROM app_settings WHERE key='aramhex_manifest'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(old) = old {
            if let Ok(old) = serde_json::from_str::<Manifest>(&old) {
                if self.seq < old.seq {
                    return Ok(false);
                }
                if self.identity() == old.identity() {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}

#[derive(Deserialize)]
struct Hero {
    name: String,
    title: String,
    games: u64,
    wins: u64,
    win_rate: f64,
    rank: u16,
}
#[derive(Deserialize)]
struct Augment {
    name: String,
    rarity: String,
}
#[derive(Deserialize)]
struct Stats {
    games: u64,
    wins: u64,
    win_rate: f64,
    low_sample: bool,
}
#[derive(Deserialize)]
struct Excluded {
    name: String,
    reason: String,
}
#[derive(Deserialize)]
struct Meta {
    patch: String,
    mode: String,
    snapshot_at: String,
    #[serde(default)]
    excluded_augments: BTreeMap<String, Excluded>,
}

struct Dataset {
    manifest: Manifest,
    heroes: BTreeMap<String, Hero>,
    hero_augments: BTreeMap<String, BTreeMap<String, Stats>>,
    augments: BTreeMap<String, Augment>,
    meta: Meta,
}

struct Rating {
    grade: Option<String>,
    delta_pp: Option<f64>,
    low_sample: bool,
    reason: String,
}

fn rate(wins: u64, games: u64, baseline: f64, low_sample: bool) -> Rating {
    if games == 0 {
        return Rating {
            grade: None,
            delta_pp: None,
            low_sample: true,
            reason: "暂无可靠数据".into(),
        };
    }
    let delta = ((wins as f64 + baseline * 300.0) / (games as f64 + 300.0) - baseline) * 100.0;
    let (grade, advice) = if delta >= 2.0 {
        ("S", "优先考虑")
    } else if delta >= 0.0 {
        ("A", "可以选择")
    } else {
        ("B", "收益偏低，可考虑刷新")
    };
    let low_sample = low_sample || games < 1000;
    let reason = if low_sample {
        format!("样本偏少，仅供参考；平滑后较英雄平均 {delta:+.1} 个百分点")
    } else {
        format!("{advice}；平滑后较英雄平均 {delta:+.1} 个百分点")
    };
    Rating {
        grade: Some(grade.into()),
        delta_pp: Some(delta),
        low_sample,
        reason,
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 12 && id.chars().all(|c| c.is_ascii_digit())
}
fn valid_stats(wins: u64, games: u64, win: f64) -> bool {
    games > 0
        && games <= 10_000_000_000
        && wins <= games
        && win.is_finite()
        && (0.0..=1.0).contains(&win)
        && (wins as f64 / games as f64 - win).abs() < 0.00001
}

impl Dataset {
    fn parse(manifest: &[u8], contents: [&[u8]; 4]) -> Result<Self, String> {
        let manifest: Manifest = serde_json::from_slice(manifest).map_err(|e| e.to_string())?;
        manifest.validate()?;
        for (name, bytes) in FILES.iter().zip(contents) {
            manifest.verify(name, bytes)?;
        }
        let data = Self {
            manifest,
            heroes: serde_json::from_slice(contents[0]).map_err(|e| e.to_string())?,
            hero_augments: serde_json::from_slice(contents[1]).map_err(|e| e.to_string())?,
            augments: serde_json::from_slice(contents[2]).map_err(|e| e.to_string())?,
            meta: serde_json::from_slice(contents[3]).map_err(|e| e.to_string())?,
        };
        data.validate()?;
        Ok(data)
    }

    fn validate(&self) -> Result<(), String> {
        self.manifest.validate()?;
        if !(100..=300).contains(&self.heroes.len())
            || !(100..=1000).contains(&self.augments.len())
            || self.meta.patch != self.manifest.patch
            || self.meta.mode != self.manifest.mode
            || self.meta.snapshot_at != self.manifest.snapshot_at
            || self.hero_augments.len() != self.heroes.len()
        {
            return Err("AramHex 快照不完整或版本不一致".into());
        }
        let mut ranks = HashSet::new();
        for (id, h) in &self.heroes {
            if !valid_id(id)
                || h.name.trim().is_empty()
                || h.name.len() > 200
                || h.title.len() > 200
                || !valid_stats(h.wins, h.games, h.win_rate)
                || h.rank == 0
                || h.rank as usize > self.heroes.len()
                || !ranks.insert(h.rank)
            {
                return Err(format!("AramHex 英雄统计无效：{id}"));
            }
            let stats = self
                .hero_augments
                .get(id)
                .ok_or("AramHex 缺少英雄海克斯统计")?;
            if stats.is_empty() {
                return Err("AramHex 英雄海克斯统计为空".into());
            }
            for (aid, s) in stats {
                if !self.augments.contains_key(aid)
                    || !valid_stats(s.wins, s.games, s.win_rate)
                    || s.games > h.games
                    || s.wins > h.wins
                    || s.games - s.wins > h.games - h.wins
                {
                    return Err(format!("AramHex 海克斯统计无效：{id}/{aid}"));
                }
            }
        }
        for (id, a) in &self.augments {
            if !valid_id(id)
                || a.name.trim().is_empty()
                || a.name.len() > 400
                || !matches!(a.rarity.as_str(), "silver" | "gold" | "prismatic")
            {
                return Err(format!("AramHex 海克斯目录无效：{id}"));
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn needs_update(&self, db: &Connection) -> Result<bool, String> {
        self.manifest.needs_update(db)
    }

    fn import(&self, db: &mut Connection) -> Result<(), String> {
        self.validate()?;
        let tx = db.transaction().map_err(|e| e.to_string())?;
        let write = || -> rusqlite::Result<()> {
            tx.execute("DELETE FROM aramhex_augment_stats", [])?;
            tx.execute("DELETE FROM aramhex_heroes", [])?;
            tx.execute("DELETE FROM aramhex_augments", [])?;
            tx.execute("DELETE FROM champion_rankings", [])?;
            for (aid, a) in &self.augments {
                if aid == "3176" {
                    continue;
                }
                let excluded = self
                    .meta
                    .excluded_augments
                    .get(aid)
                    .map(|e| e.reason.as_str());
                tx.execute("INSERT INTO aramhex_augments (augment_id,name,excluded_reason) VALUES (?1,?2,?3)", params![aid,a.name,excluded])?;
            }
            // Some deliberately excluded augments have no statistics/catalog
            // row. Keep their names recognizable, with no grade or win rate.
            for (aid, excluded) in &self.meta.excluded_augments {
                if aid != "3176" {
                    tx.execute("INSERT OR IGNORE INTO aramhex_augments (augment_id,name,excluded_reason) VALUES (?1,?2,?3)", params![aid,excluded.name,excluded.reason])?;
                }
            }
            for (id, h) in &self.heroes {
                // The existing seed uses English IDs and Chinese titles. Resolve
                // before renaming, so LCU/live-client lookups keep the same ID.
                let canonical: String = tx.query_row("SELECT champion_id FROM champions WHERE name IN (?1,?2) OR champion_id=?3 ORDER BY CASE WHEN champion_id=?3 THEN 0 ELSE 1 END LIMIT 1",
                    params![h.name,h.title,id], |r| r.get(0)).optional()?.unwrap_or_else(|| id.clone());
                tx.execute("INSERT INTO champions (champion_id,name,patch) VALUES (?1,?2,?3) ON CONFLICT(champion_id) DO UPDATE SET patch=excluded.patch", params![canonical,h.title,self.manifest.patch])?;
                tx.execute("INSERT INTO aramhex_heroes (champion_id,source_id,name,title,win_rate,games) VALUES (?1,?2,?3,?4,?5,?6)", params![canonical,id,h.name,h.title,h.win_rate,h.games])?;
                tx.execute("INSERT INTO champion_rankings (champion_id,name,position,total,badge_tier,patch,source,updated_at) VALUES (?1,?2,?3,?4,NULL,?5,?6,?7)", params![canonical,h.title,h.rank,self.heroes.len(),self.manifest.patch,SOURCE,self.manifest.snapshot_at])?;
                for (aid, s) in &self.hero_augments[id] {
                    if aid == "3176" || self.meta.excluded_augments.contains_key(aid) {
                        continue;
                    }
                    let rating = rate(s.wins, s.games, h.win_rate, s.low_sample);
                    tx.execute("INSERT INTO aramhex_augment_stats (champion_id,augment_id,win_rate,games,grade,delta_pp,low_sample,reason) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)", params![canonical,aid,s.win_rate,s.games,rating.grade,rating.delta_pp,rating.low_sample,rating.reason])?;
                }
            }
            for key in ["rankings", "augment_grades"] {
                tx.execute("INSERT INTO snapshot_metadata (key,source,patch,updated_at,stale) VALUES (?1,?2,?3,?4,0) ON CONFLICT(key) DO UPDATE SET source=excluded.source,patch=excluded.patch,updated_at=excluded.updated_at,stale=0", params![key,SOURCE,self.manifest.patch,self.manifest.snapshot_at])?;
            }
            tx.execute("INSERT INTO app_settings (key,value) VALUES ('aramhex_manifest',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [self.manifest.identity()])?;
            Ok(())
        };
        write().map_err(|e| format!("导入 AramHex 数据失败：{e}"))?;
        tx.commit().map_err(|e| e.to_string())
    }
}

pub fn migrate(db: &Connection) -> rusqlite::Result<()> {
    let tx = db.unchecked_transaction()?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS aramhex_heroes (champion_id TEXT PRIMARY KEY,source_id TEXT UNIQUE NOT NULL,name TEXT NOT NULL,title TEXT NOT NULL,win_rate REAL NOT NULL,games INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS aramhex_augments (augment_id TEXT PRIMARY KEY,name TEXT NOT NULL,excluded_reason TEXT);
        CREATE TABLE IF NOT EXISTS aramhex_augment_stats (champion_id TEXT NOT NULL,augment_id TEXT NOT NULL,win_rate REAL NOT NULL,games INTEGER NOT NULL,grade TEXT,delta_pp REAL,low_sample INTEGER NOT NULL,reason TEXT NOT NULL,PRIMARY KEY(champion_id,augment_id));")?;
    let columns = {
        let mut statement = tx.prepare("PRAGMA table_info(aramhex_heroes)")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
        rows.collect::<rusqlite::Result<HashSet<_>>>()?
    };
    for column in ["name", "title"] {
        if !columns.contains(column) {
            tx.execute_batch(&format!(
                "ALTER TABLE aramhex_heroes ADD COLUMN {column} TEXT NOT NULL DEFAULT '';"
            ))?;
        }
    }
    let missing_names: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM aramhex_heroes WHERE name='' OR title='')",
        [],
        |r| r.get(0),
    )?;
    if missing_names {
        // Backfill only identity fields. A newer downloaded snapshot's counts,
        // ratings and manifest must survive this schema-only upgrade unchanged.
        let heroes: BTreeMap<String, Hero> =
            serde_json::from_slice(include_bytes!("../../../data/seed/aramhex/heroes.json"))
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        for (source_id, hero) in heroes {
            tx.execute("UPDATE aramhex_heroes SET name=CASE WHEN name='' THEN ?1 ELSE name END,title=CASE WHEN title='' THEN ?2 ELSE title END WHERE source_id=?3 AND (name='' OR title='')", params![hero.name,hero.title,source_id])?;
        }
        // A future champion absent from the bundle still keeps its cached ID
        // and known local name until the next full data update supplies aliases.
        tx.execute("UPDATE aramhex_heroes SET title=COALESCE((SELECT name FROM champions WHERE champions.champion_id=aramhex_heroes.champion_id),champion_id) WHERE title=''", [])?;
        tx.execute("UPDATE aramhex_heroes SET name=title WHERE name=''", [])?;
    }
    tx.commit()
}

pub fn seed(db: &mut Connection) -> Result<(), String> {
    let data = Dataset::parse(
        include_bytes!("../../../data/seed/aramhex/latest.json"),
        [
            include_bytes!("../../../data/seed/aramhex/heroes.json"),
            include_bytes!("../../../data/seed/aramhex/hero_augments.json"),
            include_bytes!("../../../data/seed/aramhex/augments.json"),
            include_bytes!("../../../data/seed/aramhex/meta.json"),
        ],
    )?;
    if data.manifest.needs_update(db)? {
        data.import(db)?;
    }
    Ok(())
}

async fn download(client: &Client, url: &str, limit: usize) -> Result<Vec<u8>, String> {
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("{e:?}"))?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    if response.content_length().is_some_and(|n| n > limit as u64) {
        return Err("AramHex 文件超过大小限制".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| format!("{e:?}"))? {
        if bytes.len() + chunk.len() > limit {
            return Err("AramHex 文件超过大小限制".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn first_verified<T, F, Fut, V>(routes: &[T], fetch: F, verify: V) -> Result<Vec<u8>, String>
where
    T: Clone + std::fmt::Debug,
    F: Fn(T) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<u8>, String>>,
    V: Fn(&[u8]) -> Result<(), String>,
{
    let mut failures = Vec::new();
    for route in routes {
        let attempt = match fetch(route.clone()).await {
            Ok(bytes) => verify(&bytes).map(|()| bytes),
            Err(error) => Err(error),
        };
        match attempt {
            Ok(bytes) => return Ok(bytes),
            Err(error) => {
                eprintln!("AramHex 下载通道不可用，尝试备用：{route:?}：{error}");
                failures.push(format!("{route:?}：{error}"));
            }
        }
    }
    Err(format!(
        "所有 AramHex 下载通道均失败：{}",
        failures.join("；")
    ))
}

#[derive(Clone, Debug)]
struct DownloadRoute {
    url: String,
    direct: bool,
}

fn download_routes(reference: &str, path: &str) -> Vec<DownloadRoute> {
    // The author's official data mirror serves the same manifest and files.
    // Mutable mirror paths are accepted only against manifest sizes/hashes.
    let mut routes = vec![
        DownloadRoute {
            url: format!("https://aramhex.com/u/data/{path}"),
            direct: true,
        },
        DownloadRoute {
            url: format!("https://aramhex.com/u/data/{path}"),
            direct: false,
        },
    ];
    let urls = [
        format!("{BASE}/{reference}/{path}"),
        format!("https://fastly.jsdelivr.net/gh/czgreat/aramhex-data@{reference}/{path}"),
        format!("https://gcore.jsdelivr.net/gh/czgreat/aramhex-data@{reference}/{path}"),
    ];
    routes.extend(
        [false, true]
            .into_iter()
            .flat_map(|direct| {
                urls.iter().map(move |url| DownloadRoute {
                    url: url.clone(),
                    direct,
                })
            })
            .collect::<Vec<_>>(),
    );
    routes
}

struct DownloadClients {
    system: Client,
    direct: Client,
}

impl DownloadClients {
    fn new() -> Result<Self, String> {
        let builder = || {
            Client::builder()
                .user_agent("ARAM-Companion/0.1 (+AramHex public data)")
                .connect_timeout(Duration::from_secs(4))
                .timeout(Duration::from_secs(25))
        };
        Ok(Self {
            system: builder().build().map_err(|e| format!("{e:?}"))?,
            // This fallback affects only these public aggregate downloads;
            // never change Windows proxy settings or game/AI clients.
            direct: builder().no_proxy().build().map_err(|e| format!("{e:?}"))?,
        })
    }

    async fn fetch<V>(
        &self,
        reference: &str,
        path: &str,
        limit: usize,
        verify: V,
    ) -> Result<Vec<u8>, String>
    where
        V: Fn(&[u8]) -> Result<(), String>,
    {
        let routes = download_routes(reference, path);
        first_verified(
            &routes,
            |route| async move {
                let client = if route.direct {
                    &self.direct
                } else {
                    &self.system
                };
                download(client, &route.url, limit).await
            },
            verify,
        )
        .await
    }
}

async fn sync(app: &AppHandle) -> Result<bool, String> {
    let clients = DownloadClients::new()?;
    let bytes = clients
        .fetch("main", "latest.json", 128 * 1024, |bytes| {
            let manifest: Manifest = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
            manifest.validate()
        })
        .await?;
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    manifest.validate()?;
    {
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(|_| "数据库锁定失败")?;
        if !manifest.needs_update(&db)? {
            db.execute(
                "UPDATE snapshot_metadata SET stale=0 WHERE source=?1",
                [SOURCE],
            )
            .map_err(|e| e.to_string())?;
            return Ok(false);
        }
    }
    let mut contents = Vec::new();
    for name in FILES {
        contents.push(
            clients
                .fetch(
                    &format!("data-v{}", manifest.seq),
                    &format!("{}/{name}", manifest.path),
                    manifest.files[name].size,
                    |bytes| manifest.verify(name, bytes),
                )
                .await?,
        );
    }
    let data = Dataset::parse(
        &bytes,
        [&contents[0], &contents[1], &contents[2], &contents[3]],
    )?;
    let state = app.state::<AppState>();
    let mut db = state.db.lock().map_err(|_| "数据库锁定失败")?;
    data.import(&mut db)?;
    Ok(true)
}

pub fn start_sync(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let wait = match sync(&app).await {
                Ok(changed) => {
                    if changed {
                        eprintln!("AramHex 统计数据已自动更新");
                    } else {
                        eprintln!("AramHex 数据检查完成，当前缓存无需更新");
                    }
                    CHECK_INTERVAL
                }
                Err(error) => {
                    eprintln!("AramHex 同步失败，保留本地缓存：{error}");
                    let state = app.state::<AppState>();
                    if let Ok(db) = state.db.lock() {
                        let _ = db.execute(
                            "UPDATE snapshot_metadata SET stale=1 WHERE source=?1",
                            [SOURCE],
                        );
                    };
                    Duration::from_secs(15 * 60)
                }
            };
            let _ = app.emit("rankings:updated", ());
            tokio::time::sleep(wait).await;
        }
    });
}

#[cfg(test)]
mod tests;
