pub mod snapshot;

use rusqlite::Row;

pub fn resolve_champion_identity(
    db: &rusqlite::Connection,
    id: &str,
    name_hint: Option<&str>,
) -> rusqlite::Result<Option<(String, String)>> {
    use rusqlite::OptionalExtension;
    db.query_row(
        "SELECT c.champion_id,c.name FROM champions c
        LEFT JOIN aramhex_heroes h ON h.champion_id=c.champion_id
        WHERE c.champion_id=?1 COLLATE NOCASE OR c.name=?1 COLLATE NOCASE
            OR h.source_id=?1 OR h.name=?1 OR h.title=?1
            OR (?2 IS NOT NULL AND (c.name=?2 COLLATE NOCASE OR h.name=?2 OR h.title=?2))
        ORDER BY CASE WHEN c.champion_id=?1 COLLATE NOCASE THEN 0 ELSE 1 END LIMIT 1",
        rusqlite::params![id, name_hint],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
}

pub fn load_augment_advice(
    db: &rusqlite::Connection,
    champion_id: &str,
) -> Result<Vec<crate::domain::AugmentGradeRow>, String> {
    use rusqlite::OptionalExtension;
    let source: Option<String> = db
        .query_row(
            "SELECT source FROM snapshot_metadata WHERE key='augment_grades'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let sql = if source.as_deref().is_some_and(|s| s.starts_with("AramHex")) {
        "SELECT h.champion_id,a.augment_id,a.name,
            CASE WHEN a.excluded_reason IS NULL THEN s.grade END,
            COALESCE(a.excluded_reason,s.reason,'暂无可靠数据'),
            (SELECT patch FROM snapshot_metadata WHERE key='augment_grades'),
            s.win_rate,s.games,s.delta_pp,COALESCE(s.low_sample,1)
         FROM aramhex_heroes h CROSS JOIN aramhex_augments a
         LEFT JOIN aramhex_augment_stats s ON s.champion_id=h.champion_id AND s.augment_id=a.augment_id
         WHERE h.champion_id=?1 COLLATE NOCASE OR h.source_id=?1
         ORDER BY CASE s.grade WHEN 'S' THEN 0 WHEN 'A' THEN 1 WHEN 'B' THEN 2 ELSE 3 END,s.delta_pp DESC,a.name"
    } else {
        "SELECT champion_id,augment_id,name,grade,reason,patch,NULL,NULL,NULL,0 FROM champion_augment_grades
         WHERE champion_id=?1 COLLATE NOCASE ORDER BY CASE grade WHEN 'S' THEN 0 WHEN 'A' THEN 1 ELSE 2 END,name"
    };
    let mut statement = db.prepare(sql).map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([champion_id], |r| {
            Ok(crate::domain::AugmentGradeRow {
                champion_id: r.get(0)?,
                augment_id: r.get(1)?,
                name: r.get(2)?,
                grade: r.get(3)?,
                reason: r.get(4)?,
                patch: r.get(5)?,
                win_rate: r.get(6)?,
                games: r.get(7)?,
                delta_pp: r.get(8)?,
                low_sample: r.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn map_rank_row(row: &Row<'_>) -> rusqlite::Result<crate::domain::ChampionRankRow> {
    let position: Option<u16> = row.get(2)?;
    let total: Option<u16> = row.get(3)?;
    let source: Option<String> = row.get(6)?;
    let mut badge_tier: Option<u8> = row.get(4)?;
    if badge_tier.is_none() && source.as_deref().is_some_and(|s| s.starts_with("AramHex")) {
        // App-derived rank quintiles; AramHex does not supply official tiers.
        badge_tier = position.zip(total)
            .filter(|(position,total)| *total > 0 && *position > 0 && position <= total)
            .map(|(position,total)| (((u32::from(position) - 1) * 5 / u32::from(total)) + 1) as u8);
    }
    Ok(crate::domain::ChampionRankRow {
        champion_id: row.get(0)?,
        name: row.get(1)?,
        position,
        total,
        badge_tier,
        patch: row.get(5)?,
        source,
        win_rate: None,
        updated_at: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aramhex_rank_badges_derive_five_bands_from_cached_positions() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        for (position, expected) in [(1,1),(35,1),(36,2),(60,2),(70,2),(71,3),(104,3),(105,4),(139,4),(140,5),(173,5)] {
            let rank = db.query_row("SELECT 'Vex','薇古丝',?1,173,NULL,'16.21','AramHex · 国服全体样本',NULL", [position], map_rank_row).unwrap();
            assert_eq!(rank.badge_tier, Some(expected), "position {position}");
        }
        for (position,total) in [(0,173),(174,173),(1,0)] {
            let rank = db.query_row("SELECT 'Vex','薇古丝',?1,?2,NULL,NULL,'AramHex',NULL", [position,total], map_rank_row).unwrap();
            assert_eq!(rank.badge_tier,None);
        }
        let old = db.query_row("SELECT 'Vex','薇古丝',60,173,4,NULL,'OP.GG',NULL", [], map_rank_row).unwrap();
        assert_eq!(old.badge_tier,Some(4));
        let unknown = db.query_row("SELECT 'Vex','薇古丝',60,173,NULL,NULL,'Manual',NULL", [], map_rank_row).unwrap();
        assert_eq!(unknown.badge_tier,None);
    }
}
