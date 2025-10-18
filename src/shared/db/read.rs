use anyhow::{Context as _, Result, anyhow, bail};
use rusqlite::{Connection, Error, OptionalExtension as _, params, types::Value};

use crate::{
    error::UserError,
    shared::types::{Bingo, BingoKind, SqlResponse},
};

pub fn complete_bingo_data(conn: &mut Connection, bingo_ids: &[u8]) -> Result<Vec<Bingo>, Error> {
    let transaction = conn.transaction()?;

    let mut statement = transaction.prepare(
        "
        SELECT bingo, bingo_kind, kind_specific_id
        FROM bingo_kind_id_map
        WHERE bingo=?1
        ",
    )?;

    let bingos: Vec<Bingo> = bingo_ids
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
        .collect::<Result<_, Error>>()?;

    Ok(bingos)
}

pub fn get_current_bingo(conn: &Connection) -> Result<Option<(u8, u32, u32)>, Error> {
    conn.query_one(
        "
        SELECT current_bingo, current_bingo_starts, current_bingo_ends
        FROM current_bingo_global WHERE id=1
        ",
        [],
        |row| {
            Ok((
                row.get::<_, Option<u8>>("current_bingo")?,
                row.get::<_, Option<u32>>("current_bingo_starts")?,
                row.get::<_, Option<u32>>("current_bingo_ends")?,
            ))
        },
    )
    .optional()
    // flatten
    .map(|opt| opt.and_then(|(id, start, end)| Some((id?, start?, end?))))
}

pub fn get_is_network_bingo(conn: &Connection) -> Result<Option<bool>, Error> {
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

pub fn raw_query_readonly(conn: &mut Connection, sql: String) -> Result<SqlResponse> {
    let mut statement = conn
        .prepare(&sql)
        .context(UserError(anyhow!("Invalid SQL")))?;

    if !statement.readonly() {
        bail!(UserError(anyhow!(
            "The provided SQL statement isn't read-only"
        )))
    }

    let upper_sql = sql.trim_start().to_uppercase();
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
