use rusqlite::{Connection, Result, params};

use crate::db::DbRequest;
use crate::role::types::NetworkBingo;
use crate::shared::{db::GetCurrentBingo, types::BitSet};

// NOTE: Before caching any data, the current bingo should be updated. If data is cached while the
// current bingo is outdated, issues could arise.

pub struct CacheCompletions {
    pub uuid: String,
    pub completions: BitSet,
}
impl DbRequest for CacheCompletions {
    type ReturnValue = Result<()>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let (current_bingo, _, _) = GetCurrentBingo.execute(conn)?.unwrap_or_default();

        conn.execute(
            "
            INSERT OR REPLACE INTO role_completions_cache (uuid, updated_after_bingo, bingo_set)
            VALUES (?1, ?2, ?3)
            ",
            params![self.uuid, current_bingo, self.completions.data],
        )?;
        Ok(())
    }
}

pub struct CacheNetworkBingos {
    pub uuid: String,
    pub completions: BitSet,
}
impl DbRequest for CacheNetworkBingos {
    type ReturnValue = Result<()>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let current_network_bingo =
            *NetworkBingo::ALL.last().unwrap_or(&NetworkBingo::Unknown) as u8;

        conn.execute(
            "
            INSERT OR REPLACE INTO role_network_bingo_cache (uuid, updated_after_bingo, bingo_set)
            VALUES (?1, ?2, ?3)
            ",
            params![self.uuid, current_network_bingo, self.completions.data],
        )?;
        Ok(())
    }
}

pub struct CacheBingoRank {
    pub uuid: String,
    pub rank: u8,
}
impl DbRequest for CacheBingoRank {
    type ReturnValue = Result<()>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let (current_bingo, _, _) = GetCurrentBingo.execute(conn)?.unwrap_or_default();

        conn.execute(
            "
            INSERT OR REPLACE INTO role_bingo_rank_cache (uuid, updated_after_bingo, rank)
            VALUES (?1, ?2, ?3)
            ",
            params![self.uuid, current_bingo, self.rank],
        )?;
        Ok(())
    }
}

pub struct CacheImmortal {
    pub uuid: String,
    pub has_achieved: bool,
}
impl DbRequest for CacheImmortal {
    type ReturnValue = Result<()>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let (current_bingo, _, _) = GetCurrentBingo.execute(conn)?.unwrap_or_default();

        conn.execute(
            "
            INSERT OR REPLACE INTO role_immortal_cache (uuid, updated_after_bingo, has_achieved)
            VALUES (?1, ?2, ?3)
            ",
            params![self.uuid, current_bingo, self.has_achieved],
        )?;
        Ok(())
    }
}

pub struct CacheHypixelPlayerEndpoint {
    pub uuid: String,
    pub timestamp: i64,
    pub json: String,
}
impl DbRequest for CacheHypixelPlayerEndpoint {
    type ReturnValue = Result<()>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        conn.execute(
            "
            INSERT OR REPLACE INTO role_player_endpoint_cache (uuid, timestamp, json)
            VALUES (?1, ?2, ?3)
            ",
            params![self.uuid, self.timestamp, self.json],
        )?;
        Ok(())
    }
}
