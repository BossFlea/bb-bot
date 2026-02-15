use rusqlite::{Connection, OptionalExtension as _, Result, params};

use crate::db::DbRequest;
use crate::role::types::NetworkBingo;
use crate::shared::{db::GetCurrentBingo, types::BitSet};

// NOTE: Before reading any cached data, the current bingo should be updated. If cached data is
// read while the current bingo is outdated, issues could arise.

pub struct CachedCompletions {
    pub uuid: String,
}
impl DbRequest for CachedCompletions {
    type ReturnValue = Result<Option<BitSet>>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let cached = conn
            .query_one(
                "
                SELECT updated_after_bingo, bingo_set
                FROM role_completions_cache
                WHERE uuid=?1
                ",
                params![self.uuid],
                |row| {
                    Ok((
                        row.get("updated_after_bingo")?,
                        row.get::<_, Option<_>>("bingo_set")?.unwrap_or_default(),
                    ))
                },
            )
            .optional()?;

        let (current_bingo, _, _) = GetCurrentBingo.execute(conn)?.unwrap_or_default();

        if let Some((updated_after, bingo_bytes)) = cached {
            if current_bingo.get_id() > updated_after {
                // invalid, delete cache entry
                conn.execute(
                    "
                    DELETE FROM role_completions_cache
                    WHERE uuid=?1
                    ",
                    params![self.uuid],
                )?;
                Ok(None)
            } else {
                Ok(Some(BitSet::from_bytes(bingo_bytes)))
            }
        } else {
            Ok(None)
        }
    }
}

pub struct CachedNetworkBingos {
    pub uuid: String,
}
impl DbRequest for CachedNetworkBingos {
    type ReturnValue = Result<Option<BitSet>>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let cached = conn
            .query_one(
                "
                SELECT updated_after_bingo, bingo_set
                FROM role_network_bingo_cache
                WHERE uuid=?1
                ",
                params![self.uuid],
                |row| {
                    Ok((
                        row.get("updated_after_bingo")?,
                        row.get::<_, Option<_>>("bingo_set")?.unwrap_or_default(),
                    ))
                },
            )
            .optional()?;

        let current_network_bingo =
            *NetworkBingo::ALL.last().unwrap_or(&NetworkBingo::Unknown) as u8;

        if let Some((updated_after, bingo_bytes)) = cached {
            if current_network_bingo > updated_after {
                // invalid, delete cache entry
                conn.execute(
                    "
                    DELETE FROM role_network_bingo_cache
                    WHERE uuid=?1
                    ",
                    params![self.uuid],
                )?;
                Ok(None)
            } else {
                Ok(Some(BitSet::from_bytes(bingo_bytes)))
            }
        } else {
            Ok(None)
        }
    }
}

pub struct CachedBingoRank {
    pub uuid: String,
}
impl DbRequest for CachedBingoRank {
    type ReturnValue = Result<Option<u8>>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let cached = conn
            .query_one(
                "
                SELECT updated_after_bingo, rank
                FROM role_bingo_rank_cache
                WHERE uuid=?1
                ",
                params![self.uuid],
                |row| {
                    Ok((
                        row.get("updated_after_bingo")?,
                        row.get::<_, Option<_>>("rank")?.unwrap_or_default(),
                    ))
                },
            )
            .optional()?;

        let (current_bingo, _, _) = GetCurrentBingo.execute(conn)?.unwrap_or_default();

        if let Some((updated_after, rank)) = cached {
            if current_bingo.get_id() > updated_after {
                // invalid, delete cache entry
                conn.execute(
                    "
                    DELETE FROM role_bingo_rank_cache
                    WHERE uuid=?1
                    ",
                    params![self.uuid],
                )?;
                Ok(None)
            } else {
                Ok(Some(rank))
            }
        } else {
            Ok(None)
        }
    }
}

pub struct CachedImmortal {
    pub uuid: String,
}
impl DbRequest for CachedImmortal {
    type ReturnValue = Result<Option<bool>>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let cached: Option<(_, bool)> = conn
            .query_one(
                "
                SELECT updated_after_bingo, has_achieved
                FROM role_immortal_cache
                WHERE uuid=?1
                ",
                params![self.uuid],
                |row| {
                    Ok((
                        row.get("updated_after_bingo")?,
                        row.get::<_, Option<_>>("has_achieved")?.unwrap_or_default(),
                    ))
                },
            )
            .optional()?;

        let (current_bingo, _, _) = GetCurrentBingo.execute(conn)?.unwrap_or_default();

        if let Some((updated_after, has_achieved)) = cached {
            if current_bingo.get_id() > updated_after && !has_achieved {
                // invalid, delete cache entry
                conn.execute(
                    "
                    DELETE FROM role_immortal_cache
                    WHERE uuid=?1
                    ",
                    params![self.uuid],
                )?;
                Ok(None)
            } else {
                Ok(Some(has_achieved))
            }
        } else {
            Ok(None)
        }
    }
}

pub struct CachedHypixelPlayerEndpoint {
    pub uuid: String,
}
impl DbRequest for CachedHypixelPlayerEndpoint {
    type ReturnValue = Result<Option<(i64, String)>>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let cached = conn
            .query_one(
                "
                SELECT timestamp, json
                FROM role_player_endpoint_cache
                WHERE uuid=?1
                ",
                params![self.uuid],
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
                    DELETE FROM role_player_endpoint_cache
                    WHERE uuid=?1
                    ",
                    params![self.uuid],
                )?;
                Ok(None)
            } else {
                Ok(Some((timestamp, json)))
            }
        } else {
            Ok(None)
        }
    }
}
