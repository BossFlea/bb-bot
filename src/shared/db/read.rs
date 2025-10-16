use rusqlite::{Connection, Error, OptionalExtension as _, params};

use crate::shared::types::{Bingo, BingoKind};

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
