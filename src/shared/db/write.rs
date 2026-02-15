use poise::serenity_prelude::EmojiId;
use rusqlite::{Connection, Result, params};

use crate::db::DbRequest;
use crate::shared::types::{Bingo, BingoKind};

pub struct AddBingoMapping {
    pub bingo_id: u8,
    pub bingo_kind: BingoKind,
}
impl DbRequest for AddBingoMapping {
    type ReturnValue = Result<Bingo>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let kind_specific_id = match self.bingo_kind {
            BingoKind::Normal => self.bingo_id,
            _ => {
                let mut statement = conn.prepare(
                    "SELECT kind_specific_id FROM bingo_kind_id_map WHERE bingo_kind=?1",
                )?;
                let kind_bingos: Vec<u8> = statement
                    .query_map(params![self.bingo_kind as u8], |row| {
                        row.get("kind_specific_id")
                    })?
                    .collect::<Result<_>>()?;

                kind_bingos.iter().max().map(|id| id + 1).unwrap_or(0)
            }
        };

        conn.execute(
            "
            INSERT OR IGNORE INTO bingo_kind_id_map (bingo, bingo_kind, kind_specific_id)
            VALUES (?1, ?2, ?3)
            ",
            params![self.bingo_id, self.bingo_kind as u8, kind_specific_id],
        )?;

        Ok(Bingo::new(
            kind_specific_id,
            self.bingo_kind,
            Some(self.bingo_id),
        ))
    }
}

pub struct SetCurrentBingo {
    pub bingo_id: u8,
    pub start: i64,
    pub end: i64,
}
impl DbRequest for SetCurrentBingo {
    type ReturnValue = Result<()>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        conn.execute(
            "
            INSERT INTO current_bingo_global
            (id, current_bingo, current_bingo_starts, current_bingo_ends)
            VALUES (1, ?1, ?2, ?3)
            ON CONFLICT(id) DO UPDATE SET
                current_bingo = excluded.current_bingo,
                current_bingo_starts = excluded.current_bingo_starts,
                current_bingo_ends = excluded.current_bingo_ends
            ",
            params![self.bingo_id, self.start, self.end],
        )?;
        Ok(())
    }
}

pub struct SetIsNetworkBingo {
    pub is_active: bool,
}
impl DbRequest for SetIsNetworkBingo {
    type ReturnValue = Result<()>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        conn.execute(
            "
            INSERT INTO current_bingo_global (id, is_network_bingo)
            VALUES (1, ?1)
            ON CONFLICT(id) DO UPDATE SET
                is_network_bingo = excluded.is_network_bingo
            ",
            params![self.is_active],
        )?;
        Ok(())
    }
}

#[allow(dead_code)] // used in sql script command (disabled)
pub struct RawBatch {
    pub sql: String,
}
impl DbRequest for RawBatch {
    type ReturnValue = Result<()>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        conn.execute_batch(&self.sql)?;
        Ok(())
    }
}

pub struct SetSplashReminder {
    pub enabled: bool,
    pub emoji: Option<EmojiId>,
    pub emoji_count: Option<u32>,
}
impl DbRequest for SetSplashReminder {
    type ReturnValue = Result<()>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let emoji_id = self.emoji.map(EmojiId::get);
        conn.execute(
            "
            INSERT INTO config_global (id, splash_reminder_enabled, splash_reminder_emoji_id, splash_reminder_emoji_count)
            VALUES (1, ?1, ?2, ?3)
            ON CONFLICT(id) DO UPDATE SET
                splash_reminder_enabled = excluded.splash_reminder_enabled,
                splash_reminder_emoji_id = excluded.splash_reminder_emoji_id,
                splash_reminder_emoji_count = excluded.splash_reminder_emoji_count
            ",
            params![self.enabled, emoji_id, self.emoji_count],
        )?;
        Ok(())
    }
}
