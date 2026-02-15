use anyhow::{Context as _, anyhow, bail};
use poise::serenity_prelude::EmojiId;
use rusqlite::{Connection, OptionalExtension as _, Result, params, types::Value};

use crate::db::DbRequest;
use crate::error::UserError;
use crate::shared::types::{Bingo, BingoKind, SqlResponse};

pub struct GetBingoData {
    pub bingo_ids: Vec<u8>,
}
impl DbRequest for GetBingoData {
    type ReturnValue = Result<Vec<Bingo>>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let transaction = conn.transaction()?;

        let mut statement = transaction.prepare(
            "
            SELECT bingo, bingo_kind, kind_specific_id
            FROM bingo_kind_id_map
            WHERE bingo=?1
            ",
        )?;

        let bingos: Vec<Bingo> = self
            .bingo_ids
            .iter()
            .map(|id| {
                let bingo = statement
                    .query_row(params![id], |row| {
                        Ok(Bingo {
                            kind_specific_id: row.get("kind_specific_id")?,
                            kind: BingoKind::from_u8(row.get("bingo_kind")?),
                            unique_id: row.get("bingo")?,
                        })
                    })
                    .optional()?;

                let bingo = bingo.unwrap_or(Bingo {
                    kind_specific_id: *id,
                    kind: BingoKind::Normal,
                    unique_id: None,
                });

                Ok(bingo)
            })
            .collect::<Result<_>>()?;

        Ok(bingos)
    }
}

pub struct GetCurrentBingo;
impl DbRequest for GetCurrentBingo {
    type ReturnValue = Result<Option<(Bingo, i64, i64)>>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let current = conn
            .query_one(
                "
                SELECT current_bingo, current_bingo_starts, current_bingo_ends
                FROM current_bingo_global WHERE id=1
                ",
                [],
                |row| {
                    Ok((
                        row.get::<_, Option<u8>>("current_bingo")?,
                        row.get::<_, Option<i64>>("current_bingo_starts")?,
                        row.get::<_, Option<i64>>("current_bingo_ends")?,
                    ))
                },
            )
            .optional()
            // flatten
            .map(|opt| opt.and_then(|(id, start, end)| Some((id?, start?, end?))))?;

        let Some((bingo_id, start, end)) = current else {
            return Ok(None);
        };

        let bingo = GetBingoData {
            bingo_ids: vec![bingo_id],
        }
        .execute(conn)?
        .pop();

        Ok(bingo.map(|b| (b, start, end)))
    }
}

pub struct GetIsNetworkBingo;
impl DbRequest for GetIsNetworkBingo {
    type ReturnValue = Result<Option<bool>>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        conn.query_one(
            "
            SELECT is_network_bingo
            FROM current_bingo_global WHERE id=1
            ",
            [],
            |row| row.get::<_, Option<bool>>("is_network_bingo"),
        )
        .optional()
        .map(|opt| opt.flatten())
    }
}

pub struct RawQueryReadonly {
    pub sql: String,
}
impl DbRequest for RawQueryReadonly {
    type ReturnValue = anyhow::Result<SqlResponse>;

    // NOTE: This implementation supports all types of SQL responses, even though (I think)
    // read-only queries always return rows.
    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let mut statement = conn
            .prepare(&self.sql)
            .context(UserError(anyhow!("Invalid SQL")))?;

        if !statement.readonly() {
            bail!(UserError(anyhow!(
                "The provided SQL statement isn't read-only"
            )))
        }

        let upper_sql = self.sql.trim_start().to_uppercase();
        let returns_rows = upper_sql.starts_with("SELECT") || upper_sql.starts_with("VALUES");

        let column_names: Vec<_> = statement
            .column_names()
            .into_iter()
            .map(String::from)
            .collect();

        if returns_rows {
            let mut rows = statement
                .query_map([], |row| {
                    let mut values = Vec::with_capacity(column_names.len());
                    for column in &column_names {
                        values.push(match row.get::<_, Value>(column.as_str())? {
                            Value::Null => "NULL".to_string(),
                            Value::Integer(i) => i.to_string(),
                            Value::Real(f) => f.to_string(),
                            Value::Text(t) => t,
                            Value::Blob(b) => format!("<blob {} bytes>", b.len()),
                        });
                    }

                    Ok(values.join(" | "))
                })?
                .collect::<Result<Vec<_>, _>>()?;

            rows = [column_names.join(" | "), String::new()]
                .into_iter()
                .chain(rows)
                .collect();
            Ok(SqlResponse::ReturnedRows(rows))
        } else {
            Ok(SqlResponse::AffectedRows(statement.execute([])?))
        }
    }
}

pub struct GetSplashReminder;
impl DbRequest for GetSplashReminder {
    type ReturnValue = Result<(bool, Option<EmojiId>, Option<u32>)>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        conn.query_one(
            "
            SELECT splash_reminder_enabled, splash_reminder_emoji_id, splash_reminder_emoji_count
            FROM config_global WHERE id=1
            ",
            [],
            |row| {
                Ok((
                    row.get::<_, bool>("splash_reminder_enabled")?,
                    row.get::<_, Option<u64>>("splash_reminder_emoji_id")?,
                    row.get::<_, Option<u32>>("splash_reminder_emoji_count")?,
                ))
            },
        )
        .optional()
        .map(|opt| {
            opt.map(|(e, id, count)| (e, id.map(EmojiId::new), count))
                .unwrap_or((false, None, None))
        })
    }
}
