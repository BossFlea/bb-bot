use rusqlite::{Connection, Error, OptionalExtension as _, params};

use crate::shared::types::BitSet;

pub fn cached_completions(
    conn: &Connection,
    uuid: &str,
    current_bingo: u8,
) -> Result<Option<BitSet>, Error> {
    let cached = conn
        .query_one(
            "
            SELECT updated_after_bingo, bingo_set
            FROM role_completions_cache
            WHERE uuid=?1
            ",
            params![uuid],
            |row| {
                Ok((
                    row.get("updated_after_bingo")?,
                    row.get::<_, Option<_>>("bingo_set")?.unwrap_or_default(),
                ))
            },
        )
        .optional()?;

    if let Some((updated_after, bingo_bytes)) = cached {
        if current_bingo > updated_after {
            // invalid, delete cache entry
            conn.execute(
                "
                DETELE FROM role_completions_cache
                WHERE uuid=?1
                ",
                params![uuid],
            )?;
            Ok(None)
        } else {
            Ok(Some(BitSet::from_bytes(bingo_bytes)))
        }
    } else {
        Ok(None)
    }
}

pub fn cached_network_bingos(
    conn: &Connection,
    uuid: &str,
    current_bingo: u8,
) -> Result<Option<BitSet>, Error> {
    let cached = conn
        .query_one(
            "
            SELECT updated_after_bingo, bingo_set
            FROM role_network_bingo_cache
            WHERE uuid=?1
            ",
            params![uuid],
            |row| {
                Ok((
                    row.get("updated_after_bingo")?,
                    row.get::<_, Option<_>>("bingo_set")?.unwrap_or_default(),
                ))
            },
        )
        .optional()?;

    if let Some((updated_after, bingo_bytes)) = cached {
        if current_bingo > updated_after {
            // invalid, delete cache entry
            conn.execute(
                "
                DETELE FROM role_network_bingo_cache
                WHERE uuid=?1
                ",
                params![uuid],
            )?;
            Ok(None)
        } else {
            Ok(Some(BitSet::from_bytes(bingo_bytes)))
        }
    } else {
        Ok(None)
    }
}

pub fn cached_bingo_rank(
    conn: &Connection,
    uuid: &str,
    current_bingo: u8,
) -> Result<Option<u8>, Error> {
    let cached = conn
        .query_one(
            "
            SELECT updated_after_bingo, rank
            FROM role_bingo_rank_cache
            WHERE uuid=?1
            ",
            params![uuid],
            |row| {
                Ok((
                    row.get("updated_after_bingo")?,
                    row.get::<_, Option<_>>("rank")?.unwrap_or_default(),
                ))
            },
        )
        .optional()?;

    if let Some((updated_after, rank)) = cached {
        if current_bingo > updated_after {
            // invalid, delete cache entry
            conn.execute(
                "
                DETELE FROM role_bingo_rank_cache
                WHERE uuid=?1
                ",
                params![uuid],
            )?;
            Ok(None)
        } else {
            Ok(Some(rank))
        }
    } else {
        Ok(None)
    }
}

pub fn cached_immortal(
    conn: &Connection,
    uuid: &str,
    current_bingo: u8,
) -> Result<Option<bool>, Error> {
    let cached: Option<(_, bool)> = conn
        .query_one(
            "
            SELECT updated_after_bingo, has_achieved
            FROM role_immortal_cache
            WHERE uuid=?1
            ",
            params![uuid],
            |row| {
                Ok((
                    row.get("updated_after_bingo")?,
                    row.get::<_, Option<_>>("has_achieved")?.unwrap_or_default(),
                ))
            },
        )
        .optional()?;

    if let Some((updated_after, has_achieved)) = cached {
        if current_bingo > updated_after && !has_achieved {
            // invalid, delete cache entry
            conn.execute(
                "
                DETELE FROM role_immortal_cache
                WHERE uuid=?1
                ",
                params![uuid],
            )?;
            Ok(None)
        } else {
            Ok(Some(has_achieved))
        }
    } else {
        Ok(None)
    }
}

pub fn cached_player_endpoint(
    conn: &Connection,
    uuid: &str,
) -> Result<Option<(i64, String)>, Error> {
    let cached = conn
        .query_one(
            "
            SELECT timestamp, json
            FROM role_player_endpoint_cache
            WHERE uuid=?1
            ",
            params![uuid],
            |row| {
                Ok((
                    row.get("timestamp")?,
                    row.get::<_, Option<_>>("json")?.unwrap_or_default(),
                ))
            },
        )
        .optional()?;

    if let Some((timestamp, json)) = cached {
        if chrono::Utc::now().timestamp() > timestamp + 60 {
            // invalid, delete cache entry
            conn.execute(
                "
                DETELE FROM role_player_endpoint_cache
                WHERE uuid=?1
                ",
                params![uuid],
            )?;
            Ok(None)
        } else {
            Ok(Some((timestamp, json)))
        }
    } else {
        Ok(None)
    }
}
