use std::collections::{HashMap, HashSet};
use std::time::Duration;

use chrono::{SecondsFormat, Utc};
use reqwest::Client;
use rusqlite::{params, Connection};
use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;

const OP_GG_ARAM_MAYHEM_URL: &str = "https://op.gg/lol/modes/aram-mayhem";
const OP_GG_SOURCE: &str = "OP.GG · ARAM: Mayhem";
const MIN_CHAMPIONS_IN_SNAPSHOT: usize = 100;
const MAX_CHAMPIONS_IN_SNAPSHOT: usize = 300;

#[derive(Clone, Debug, Deserialize)]
struct OpggChampion {
    key: String,
    name: String,
    tier: u8,
    rank: u16,
}

#[derive(Clone, Debug)]
struct OpggChampionRank {
    key: String,
    name: String,
    position: u16,
    badge_tier: u8,
}

#[derive(Clone, Debug)]
struct OpggRankings {
    patch: Option<String>,
    champions: Vec<OpggChampionRank>,
}

#[derive(Debug, Deserialize)]
struct OpggAugment {
    id: u32,
    tier: u8,
    name: Option<String>,
    key: Option<String>,
    performance: f64,
}

#[tauri::command]
pub async fn refresh_champion_augments(champion_id: String, app: AppHandle) -> Result<usize, String> {
    if champion_id.is_empty() || champion_id.len() > 40 || !champion_id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("英雄 ID 无效".to_string());
    }
    let slug = match champion_id.to_ascii_lowercase().as_str() {
        "monkeyking" => "wukong".to_string(),
        other => other.to_string(),
    };
    let url = format!("https://op.gg/zh-cn/lol/modes/aram-mayhem/{slug}/augments");
    let client = Client::builder().user_agent("ARAM-Companion/0.1")
        .timeout(Duration::from_secs(15)).build()
        .map_err(|e| format!("创建 OP.GG 网络连接失败：{e}"))?;
    let mut last_error = String::new();
    let mut html = None;
    for attempt in 0..3 {
        match client.get(&url).send().await {
            Ok(response) if response.status().is_success() => match response.text().await {
                Ok(body) => { html = Some(body); break; }
                Err(error) => last_error = format!("读取海克斯页面失败：{error}"),
            },
            Ok(response) => { last_error = format!("OP.GG 返回 HTTP {}", response.status()); },
            Err(error) => last_error = format!("访问 OP.GG 失败：{error}"),
        }
        if attempt < 2 { tokio::time::sleep(Duration::from_millis(700 * (attempt + 1) as u64)).await; }
    }
    let html = html.ok_or(last_error)?;
    let patch = parse_chinese_patch(&html).ok_or("OP.GG 页面没有可核实的补丁版本")?;
    let augments = parse_opgg_augments(&html)?;
    let state = app.state::<AppState>();
    let mut db = state.db.lock().map_err(|_| "本地数据库锁定失败")?;
    let tx = db.transaction().map_err(|e| format!("创建海克斯事务失败：{e}"))?;
    tx.execute("DELETE FROM champion_augment_grades WHERE champion_id = ?1 COLLATE NOCASE", [&champion_id])
        .map_err(|e| format!("更新海克斯缓存失败：{e}"))?;
    let mut count = 0;
    for augment in &augments {
        let grade = match augment.tier { 0 => "S", 1 => "A", 2 => "B", _ => continue };
        let id = format!("opgg_{}", augment.id);
        let name = augment.name.as_deref().ok_or("OP.GG 海克斯缺少名称")?;
        tx.execute("INSERT INTO augments (augment_id, name, patch) VALUES (?1, ?2, ?3) ON CONFLICT(augment_id) DO UPDATE SET name=excluded.name, patch=excluded.patch", params![id, name, patch])
            .map_err(|e| format!("保存海克斯名称失败：{e}"))?;
        tx.execute("INSERT INTO champion_augment_grades (champion_id, augment_id, name, grade, reason, patch) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![champion_id, id, name, grade, "OP.GG 该英雄海克斯等级", patch])
            .map_err(|e| format!("保存海克斯等级失败：{e}"))?;
        count += 1;
    }
    let key = format!("augment:{champion_id}");
    let updated = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    tx.execute("INSERT INTO snapshot_metadata (key, source, patch, updated_at, stale) VALUES (?1, ?2, ?3, ?4, 0) ON CONFLICT(key) DO UPDATE SET source=excluded.source, patch=excluded.patch, updated_at=excluded.updated_at, stale=0", params![key, OP_GG_SOURCE, patch, updated])
        .map_err(|e| format!("保存海克斯数据状态失败：{e}"))?;
    tx.commit().map_err(|e| format!("提交海克斯缓存失败：{e}"))?;
    eprintln!("OP.GG 海克斯已同步：{}，{} 条 S/A/B，补丁 {}", champion_id, count, patch);
    Ok(count)
}

fn parse_chinese_patch(html: &str) -> Option<String> {
    for (index, _) in html.match_indices("版本") {
        let prefix = &html[..index];
        let version: String = prefix.chars().rev().take_while(|c| c.is_ascii_digit() || *c == '.').collect::<String>().chars().rev().collect();
        let mut parts = version.split('.');
        if parts.next().is_some_and(|p| !p.is_empty() && p.len() <= 2) && parts.next().is_some_and(|p| !p.is_empty() && p.len() <= 2) && parts.next().is_none() { return Some(version); }
    }
    None
}

fn parse_opgg_augments(html: &str) -> Result<Vec<OpggAugment>, String> {
    let mut from = 0;
    while let Some((chunk, next)) = next_flight_chunk(html, from) {
        from = next;
        let mut pos = 0;
        while let Some(offset) = chunk[pos..].find("\"data\":[") {
            let start = pos + offset + "\"data\":".len();
            let Some(end) = matching_json_array_end(&chunk, start) else { break };
            if let Ok(rows) = serde_json::from_str::<Vec<OpggAugment>>(&chunk[start..=end]) {
                let mut ids = HashSet::new();
                if rows.len() >= 30 && rows.len() <= 300 && rows.iter().all(|r| r.id > 0 && r.tier <= 5 && r.performance.is_finite() && ids.insert(r.id) && (r.tier > 2 || (r.name.as_deref().is_some_and(|s| !s.trim().is_empty()) && r.key.as_deref().is_some_and(|s| !s.trim().is_empty())))) && rows.iter().any(|r| r.tier <= 2) { return Ok(rows); }
            }
            pos = end + 1;
        }
    }
    Err("OP.GG 没有返回完整海克斯等级，已保留原有缓存".to_string())
}

pub fn start_rank_sync(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = refresh_rankings(&app).await {
            eprintln!("OP.GG 英雄榜同步失败：{error}");
            mark_cached_rankings_stale(&app);
        }
        let _ = app.emit("rankings:updated", ());
    });
}

async fn refresh_rankings(app: &AppHandle) -> Result<(), String> {
    let client = Client::builder()
        .user_agent("ARAM-Companion/0.1 (+https://op.gg/lol/modes/aram-mayhem)")
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| format!("创建 OP.GG 网络连接失败：{error}"))?;
    let html = fetch_opgg_html(&client).await?;
    let rankings = parse_opgg_rankings(&html)?;
    validate_live_snapshot(&rankings.champions)?;
    let patch = rankings.patch.as_deref().unwrap_or("latest");
    let updated_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);

    let state = app.state::<AppState>();
    let mut db = state
        .db
        .lock()
        .map_err(|_| "本地数据库锁定失败，无法保存 OP.GG 榜单".to_string())?;
    save_rankings(
        &mut db,
        &rankings.champions,
        patch,
        &updated_at,
    )
    .map_err(|error| format!("保存 OP.GG 英雄榜失败：{error}"))?;
    eprintln!(
        "OP.GG 英雄榜已同步：{} 位英雄，补丁 {}",
        rankings.champions.len(),
        patch
    );
    Ok(())
}

async fn fetch_opgg_html(client: &Client) -> Result<String, String> {
    let mut last_error = None;
    for attempt in 0..3 {
        match client
            .get(OP_GG_ARAM_MAYHEM_URL)
            .header(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                return response
                    .text()
                    .await
                    .map_err(|error| format!("读取 OP.GG 榜单页面失败：{error}"));
            }
            Ok(response) => {
                let status = response.status();
                if status.is_client_error() && status != reqwest::StatusCode::TOO_MANY_REQUESTS {
                    return Err(format!("OP.GG 返回 HTTP {status}"));
                }
                last_error = Some(format!("OP.GG 返回 HTTP {status}"));
            }
            Err(error) => last_error = Some(format!("访问 OP.GG 失败：{error}")),
        }

        if attempt < 2 {
            tokio::time::sleep(Duration::from_millis(700 * (attempt + 1) as u64)).await;
        }
    }
    Err(last_error.unwrap_or_else(|| "OP.GG 榜单请求失败".to_string()))
}

fn parse_opgg_rankings(html: &str) -> Result<OpggRankings, String> {
    let mut search_from = 0;
    let mut best: Option<Vec<OpggChampionRank>> = None;
    while let Some((chunk, next_position)) = next_flight_chunk(html, search_from) {
        search_from = next_position;
        let mut array_search = 0;
        const CHAMPIONS_FIELD: &str = "\"champions\":";
        while let Some(field_offset) = chunk[array_search..].find(CHAMPIONS_FIELD) {
            let value_offset = array_search + field_offset + CHAMPIONS_FIELD.len();
            let Some(array_start) = chunk[value_offset..]
                .find('[')
                .map(|offset| value_offset + offset)
            else {
                break;
            };
            let Some(array_end) = matching_json_array_end(&chunk, array_start) else {
                array_search = array_start + 1;
                continue;
            };
            if let Ok(rows) = serde_json::from_str::<Vec<OpggChampion>>(&chunk[array_start..=array_end]) {
                if validate_rows(&rows).is_ok() {
                    let candidate = rows
                        .into_iter()
                        .map(|row| OpggChampionRank {
                            key: row.key,
                            name: row.name,
                            position: row.rank,
                            badge_tier: row.tier,
                        })
                        .collect::<Vec<_>>();
                    if best
                        .as_ref()
                        .is_none_or(|current| candidate.len() > current.len())
                    {
                        best = Some(candidate);
                    }
                }
            }
            array_search = array_end + 1;
        }
    }

    let champions = best.ok_or_else(|| {
        "OP.GG 页面没有返回完整的英雄名次和数字段位数据".to_string()
    })?;
    Ok(OpggRankings {
        patch: parse_patch(html),
        champions,
    })
}

fn next_flight_chunk(html: &str, from: usize) -> Option<(String, usize)> {
    const MARKER: &str = "self.__next_f.push([1,";
    let marker_offset = html.get(from..)?.find(MARKER)? + from;
    let expression_start = marker_offset + MARKER.len();
    let quote_start = expression_start + html.get(expression_start..)?.find('"')?;
    let quote_end = find_javascript_string_end(html, quote_start)?;
    let encoded = html.get(quote_start..=quote_end)?;
    let decoded = serde_json::from_str::<String>(encoded).ok()?;
    Some((decoded, quote_end + 1))
}

fn find_javascript_string_end(value: &str, quote_start: usize) -> Option<usize> {
    let bytes = value.as_bytes();
    if bytes.get(quote_start) != Some(&b'"') {
        return None;
    }
    let mut index = quote_start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return Some(index),
            _ => index += 1,
        }
    }
    None
}

fn matching_json_array_end(value: &str, array_start: usize) -> Option<usize> {
    let bytes = value.as_bytes();
    if bytes.get(array_start) != Some(&b'[') {
        return None;
    }
    let (mut depth, mut quoted, mut escaped) = (0i32, false, false);
    for (offset, byte) in bytes[array_start..].iter().enumerate() {
        if quoted {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                quoted = false;
            }
            continue;
        }
        match byte {
            b'"' => quoted = true,
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(array_start + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn validate_rows(rows: &[OpggChampion]) -> Result<(), String> {
    if rows.is_empty() || rows.len() > MAX_CHAMPIONS_IN_SNAPSHOT {
        return Err("OP.GG 英雄榜数量不合理".to_string());
    }
    let mut keys = HashSet::new();
    let mut positions = HashSet::new();
    for row in rows {
        if row.key.is_empty()
            || !row.key.chars().all(|character| character.is_ascii_alphanumeric())
            || row.name.trim().is_empty()
            || row.name.chars().count() > 100
            || !(1..=5).contains(&row.tier)
            || row.rank == 0
            || !keys.insert(row.key.to_ascii_lowercase())
            || !positions.insert(row.rank)
        {
            return Err("OP.GG 英雄榜记录缺少有效名次或段位".to_string());
        }
    }
    if positions.iter().copied().max().unwrap_or_default() as usize != rows.len()
        || !(1..=rows.len() as u16).all(|position| positions.contains(&position))
    {
        return Err("OP.GG 英雄榜名次不连续，拒绝覆盖本地缓存".to_string());
    }
    Ok(())
}

fn validate_live_snapshot(rows: &[OpggChampionRank]) -> Result<(), String> {
    if !(MIN_CHAMPIONS_IN_SNAPSHOT..=MAX_CHAMPIONS_IN_SNAPSHOT).contains(&rows.len()) {
        return Err(format!(
            "OP.GG 返回 {} 位英雄，完整榜单至少需要 {} 位",
            rows.len(), MIN_CHAMPIONS_IN_SNAPSHOT
        ));
    }
    let decoded = rows
        .iter()
        .map(|row| OpggChampion {
            key: row.key.clone(),
            name: row.name.clone(),
            tier: row.badge_tier,
            rank: row.position,
        })
        .collect::<Vec<_>>();
    validate_rows(&decoded)
}

fn parse_patch(html: &str) -> Option<String> {
    let marker = "Patch ";
    let offset = html.find(marker)? + marker.len();
    let patch = html[offset..]
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '.')
        .collect::<String>();
    (!patch.is_empty()).then_some(patch)
}

fn save_rankings(
    db: &mut Connection,
    rows: &[OpggChampionRank],
    patch: &str,
    updated_at: &str,
) -> rusqlite::Result<()> {
    let tx = db.transaction()?;
    let existing_champions = {
        let mut statement = tx.prepare("SELECT champion_id, name FROM champions")?;
        let rows = statement
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    let existing_by_key = existing_champions
        .into_iter()
        .map(|(id, name)| (normalize_champion_key(&id), (id, name)))
        .collect::<HashMap<_, _>>();
    let total = rows.len() as u16;
    for row in rows {
        let normalized_key = normalize_champion_key(&row.key);
        let (champion_id, display_name) = existing_by_key
            .get(&normalized_key)
            .cloned()
            .unwrap_or_else(|| (row.key.clone(), row.name.clone()));
        tx.execute(
            "INSERT INTO champions (champion_id, name, patch) VALUES (?1, ?2, ?3)
             ON CONFLICT(champion_id) DO UPDATE SET patch = excluded.patch",
            params![champion_id, display_name, patch],
        )?;
    }

    tx.execute("DELETE FROM champion_rankings", [])?;
    for row in rows {
        let normalized_key = normalize_champion_key(&row.key);
        let champion_id = existing_by_key
            .get(&normalized_key)
            .map(|(id, _)| id.as_str())
            .unwrap_or(row.key.as_str());
        let display_name = existing_by_key
            .get(&normalized_key)
            .map(|(_, name)| name.as_str())
            .unwrap_or(row.name.as_str());
        tx.execute(
            "INSERT INTO champion_rankings (champion_id, name, position, total, badge_tier, patch, source, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                champion_id,
                display_name,
                row.position,
                total,
                row.badge_tier,
                patch,
                OP_GG_SOURCE,
                updated_at
            ],
        )?;
    }
    tx.execute(
        "INSERT INTO snapshot_metadata (key, source, patch, updated_at, stale) VALUES ('rankings', ?1, ?2, ?3, 0)
         ON CONFLICT(key) DO UPDATE SET source = excluded.source, patch = excluded.patch, updated_at = excluded.updated_at, stale = 0",
        params![OP_GG_SOURCE, patch, updated_at],
    )?;
    tx.commit()
}

fn normalize_champion_key(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn mark_cached_rankings_stale(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Ok(db) = state.db.lock() {
        let _ = db.execute(
            "UPDATE snapshot_metadata SET stale = 1 WHERE key = 'rankings'",
            [],
        );
    };
}

#[cfg(test)]
mod tests {
    use super::{parse_chinese_patch, parse_opgg_augments, parse_opgg_rankings};

    #[test]
    fn parses_augment_tiers_and_rejects_incomplete_pages() {
        let rows: Vec<_> = (1..=35).map(|id| serde_json::json!({"id":id,"tier":(id - 1) % 6,"name":format!("海克斯{id}"),"key":format!("key{id}"),"performance":80.0})).collect();
        let rsc = format!("61:[\"$\",\"$L62\",null,{{\"data\":{}}}]", serde_json::to_string(&rows).unwrap());
        let page = format!("16.21版本<script>self.__next_f.push([1,{}]);</script>", serde_json::to_string(&rsc).unwrap());
        let parsed = parse_opgg_augments(&page).unwrap();
        assert_eq!(parsed.len(), 35);
        assert_eq!(parsed[0].tier, 0);
        assert_eq!(parsed[1].tier, 1);
        assert_eq!(parsed[2].tier, 2);
        assert_eq!(parse_chinese_patch(&page).as_deref(), Some("16.21"));
        assert!(parse_opgg_augments("<html>loading</html>").is_err());
    }

    #[test]
    #[ignore = "requires OP_GG_AUGMENT_HTML to point to a downloaded live page"]
    fn parses_downloaded_opgg_augment_page() {
        let path = std::env::var("OP_GG_AUGMENT_HTML").unwrap();
        let page = std::fs::read_to_string(path).unwrap();
        let rows = parse_opgg_augments(&page).unwrap();
        assert!(rows.len() >= 100);
        assert!(rows.iter().any(|row| row.name.as_deref() == Some("尤里卡") && row.tier == 0));
        assert!(parse_chinese_patch(&page).is_some());
        eprintln!("OP.GG 页面：{} 个海克斯，补丁 {}", rows.len(), parse_chinese_patch(&page).unwrap());
    }

    fn page_with_rankings(rsc: &str) -> String {
        let encoded = serde_json::to_string(rsc).expect("serialize RSC fixture");
        format!("<script>self.__next_f.push([1,{encoded}]);</script>")
    }

    #[test]
    fn parses_opgg_champion_rank_and_numeric_tier() {
        let page = page_with_rankings(
            r#"1:{"champions":[{"key":"aatrox","name":"Aatrox","champion_id":266,"tier":3,"rank":2},{"key":"ahri","name":"Ahri","champion_id":103,"tier":1,"rank":1}]}"#,
        );

        let rankings = parse_opgg_rankings(&page).expect("OP.GG ranking payload should parse");

        assert_eq!(rankings.champions.len(), 2);
        let aatrox = rankings
            .champions
            .iter()
            .find(|champion| champion.key == "aatrox")
            .expect("Aatrox row");
        assert_eq!(aatrox.position, 2);
        assert_eq!(aatrox.badge_tier, 3);
    }

    #[test]
    fn does_not_treat_loading_skeletons_as_a_rank_snapshot() {
        let page = "<table><caption>aram Champions Table</caption><tbody><tr><td class=animate-pulse></td></tr></tbody></table>";
        assert!(parse_opgg_rankings(page).is_err());
    }
}
