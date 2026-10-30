pub mod snapshot;

use rusqlite::Row;

pub fn map_rank_row(row: &Row<'_>) -> rusqlite::Result<crate::domain::ChampionRankRow> {
    Ok(crate::domain::ChampionRankRow {
        champion_id: row.get(0)?,
        name: row.get(1)?,
        position: row.get(2)?,
        total: row.get(3)?,
        badge_tier: row.get(4)?,
        patch: row.get(5)?,
        source: row.get(6)?,
        updated_at: row.get(7)?,
    })
}
