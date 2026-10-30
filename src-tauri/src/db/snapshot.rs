use crate::domain::DataStatus;
use rusqlite::{params, Connection, Transaction};
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RankSnapshot {
    schema_version: u16,
    patch: String,
    source: String,
    updated_at: String,
    champions: Vec<RankRecord>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RankRecord {
    champion_id: String,
    name: String,
    position: u16,
    total: u16,
    badge_tier: Option<u8>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AugmentSnapshot {
    schema_version: u16,
    patch: String,
    source: String,
    updated_at: String,
    records: Vec<AugmentRecord>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AugmentRecord {
    champion_id: String,
    champion_name: String,
    augment_id: String,
    name: String,
    grade: String,
    reason: String,
}

pub fn import_rankings_json(db: &mut Connection, contents: &str) -> Result<DataStatus, String> {
    let snapshot: RankSnapshot =
        serde_json::from_str(contents).map_err(|error| format!("排名文件格式不正确：{error}"))?;
    validate_snapshot_header(
        snapshot.schema_version,
        &snapshot.patch,
        &snapshot.source,
        &snapshot.updated_at,
    )?;
    if snapshot.champions.is_empty() || snapshot.champions.len() > 300 {
        return Err("排名快照必须包含 1 到 300 位英雄".to_string());
    }

    let mut ids = HashSet::new();
    let mut positions = HashSet::new();
    for row in &snapshot.champions {
        validate_id(&row.champion_id, "英雄")?;
        validate_text(&row.name, 100, "英雄名称")?;
        if row.total == 0 || row.total > 300 || row.position == 0 || row.position > row.total {
            return Err(format!("英雄「{}」的排名范围无效", row.name));
        }
        if row.badge_tier.is_some_and(|tier| !(1..=5).contains(&tier)) {
            return Err(format!("英雄「{}」的等级徽章必须在 1 到 5 之间", row.name));
        }
        if !ids.insert(row.champion_id.as_str()) {
            return Err(format!("排名快照中英雄 ID 重复：{}", row.champion_id));
        }
        if !positions.insert(row.position) {
            return Err(format!("排名快照中名次重复：{}", row.position));
        }
    }

    let tx = db.transaction().map_err(db_error("开始导入排名事务失败"))?;
    promote_rank_snapshot(&tx, &snapshot).map_err(db_error("写入排名快照失败"))?;
    tx.commit().map_err(db_error("提交排名快照失败"))?;
    Ok(DataStatus {
        ready: true,
        source: Some(snapshot.source),
        patch: Some(snapshot.patch),
        updated_at: Some(snapshot.updated_at),
        stale: false,
        augment_ready: false,
        augment_source: None,
        augment_patch: None,
        augment_updated_at: None,
        augment_stale: false,
    })
}

pub fn import_augment_grades_json(db: &mut Connection, contents: &str) -> Result<usize, String> {
    let snapshot: AugmentSnapshot = serde_json::from_str(contents)
        .map_err(|error| format!("海克斯分级文件格式不正确：{error}"))?;
    validate_snapshot_header(
        snapshot.schema_version,
        &snapshot.patch,
        &snapshot.source,
        &snapshot.updated_at,
    )?;
    if snapshot.records.len() > 60_000 {
        return Err("海克斯分级记录过多，最多支持 60000 条".to_string());
    }
    if snapshot.records.is_empty() {
        return Err("海克斯分级快照不能为空，当前数据不会被替换".to_string());
    }

    let mut keys = HashSet::new();
    for row in &snapshot.records {
        validate_id(&row.champion_id, "英雄")?;
        validate_id(&row.augment_id, "海克斯")?;
        validate_text(&row.champion_name, 100, "英雄名称")?;
        validate_text(&row.name, 120, "海克斯名称")?;
        validate_text(&row.reason, 400, "推荐理由")?;
        if !matches!(row.grade.as_str(), "S" | "A" | "B") {
            return Err(format!("海克斯「{}」的等级必须为 S、A 或 B", row.name));
        }
        if !keys.insert((row.champion_id.as_str(), row.augment_id.as_str())) {
            return Err(format!(
                "海克斯分级记录重复：{} / {}",
                row.champion_id, row.augment_id
            ));
        }
    }

    let record_count = snapshot.records.len();
    let tx = db
        .transaction()
        .map_err(db_error("开始导入海克斯事务失败"))?;
    promote_augment_snapshot(&tx, &snapshot).map_err(db_error("写入海克斯快照失败"))?;
    tx.commit().map_err(db_error("提交海克斯快照失败"))?;
    Ok(record_count)
}

fn validate_snapshot_header(
    version: u16,
    patch: &str,
    source: &str,
    updated_at: &str,
) -> Result<(), String> {
    if version != 1 {
        return Err(format!("不支持的数据格式版本：{version}，当前支持 1"));
    }
    validate_text(patch, 24, "版本补丁")?;
    validate_text(source, 200, "数据来源")?;
    validate_text(updated_at, 80, "更新时间")
}

fn validate_id(id: &str, label: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 80
        || !id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(format!("{label} ID 格式无效：{id}"));
    }
    Ok(())
}

fn validate_text(value: &str, max: usize, label: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > max {
        return Err(format!("{label}不能为空且不能超过 {max} 个字符"));
    }
    Ok(())
}

fn promote_rank_snapshot(tx: &Transaction<'_>, snapshot: &RankSnapshot) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM champion_rankings", [])?;
    for row in &snapshot.champions {
        tx.execute(
            "INSERT INTO champions (champion_id, name, patch) VALUES (?1, ?2, ?3)
             ON CONFLICT(champion_id) DO UPDATE SET name = excluded.name, patch = excluded.patch",
            params![row.champion_id, row.name, snapshot.patch],
        )?;
        tx.execute(
            "INSERT INTO champion_rankings (champion_id, name, position, total, badge_tier, patch, source, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![row.champion_id, row.name, row.position, row.total, row.badge_tier, snapshot.patch, snapshot.source, snapshot.updated_at],
        )?;
    }
    tx.execute(
        "INSERT INTO snapshot_metadata (key, source, patch, updated_at, stale) VALUES ('rankings', ?1, ?2, ?3, 0)
         ON CONFLICT(key) DO UPDATE SET source = excluded.source, patch = excluded.patch, updated_at = excluded.updated_at, stale = 0",
        params![snapshot.source, snapshot.patch, snapshot.updated_at],
    )?;
    Ok(())
}

fn promote_augment_snapshot(
    tx: &Transaction<'_>,
    snapshot: &AugmentSnapshot,
) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM champion_augment_grades", [])?;
    for row in &snapshot.records {
        tx.execute(
            "INSERT INTO champions (champion_id, name, patch) VALUES (?1, ?2, ?3)
             ON CONFLICT(champion_id) DO UPDATE SET name = excluded.name, patch = excluded.patch",
            params![row.champion_id, row.champion_name, snapshot.patch],
        )?;
        tx.execute(
            "INSERT INTO augments (augment_id, name, patch) VALUES (?1, ?2, ?3)
             ON CONFLICT(augment_id) DO UPDATE SET name = excluded.name, patch = excluded.patch",
            params![row.augment_id, row.name, snapshot.patch],
        )?;
        tx.execute(
            "INSERT INTO champion_augment_grades (champion_id, augment_id, name, grade, reason, patch)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![row.champion_id, row.augment_id, row.name, row.grade, row.reason, snapshot.patch],
        )?;
    }
    tx.execute(
        "INSERT INTO snapshot_metadata (key, source, patch, updated_at, stale) VALUES ('augment_grades', ?1, ?2, ?3, 0)
         ON CONFLICT(key) DO UPDATE SET source = excluded.source, patch = excluded.patch, updated_at = excluded.updated_at, stale = 0",
        params![snapshot.source, snapshot.patch, snapshot.updated_at],
    )?;
    Ok(())
}

fn db_error(context: &'static str) -> impl FnOnce(rusqlite::Error) -> String {
    move |error| format!("{context}：{error}")
}
