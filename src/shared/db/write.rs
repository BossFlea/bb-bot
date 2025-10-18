use anyhow::Result;
use rusqlite::{Connection, Error, params};

use crate::shared::types::{Bingo, BingoKind};

pub fn add_new_bingo(
    conn: &mut Connection,
    bingo_id: u8,
    bingo_kind: &BingoKind,
) -> Result<Bingo, Error> {
    let kind_specific_id = match bingo_kind {
        BingoKind::Normal => bingo_id,
        _ => {
            let mut statement =
                conn.prepare("SELECT kind_specific_id FROM bingo_kind_id_map WHERE bingo_kind=?1")?;
            let kind_bingos: Vec<u8> = statement
                .query_map(params![*bingo_kind as u8], |row| {
                    row.get("kind_specific_id")
                })?
                .collect::<Result<_, Error>>()?;

            kind_bingos.iter().max().map(|id| id + 1).unwrap_or(0)
        }
    };

    conn.execute(
        "
        INSERT OR IGNORE INTO bingo_kind_id_map (bingo, bingo_kind, kind_specific_id)
        VALUES (?1, ?2, ?3)
        ",
        params![bingo_id, *bingo_kind as u8, kind_specific_id],
    )?;

    Ok(Bingo::new(kind_specific_id, *bingo_kind, Some(bingo_id)))
}

pub fn set_current_bingo(
    conn: &mut Connection,
    bingo_id: u8,
    start: u32,
    end: u32,
) -> Result<(), Error> {
    conn.execute(
        "
        INSERT INTO current_bingo_global (id, current_bingo, current_bingo_starts, current_bingo_ends)
        VALUES (1, ?1, ?2, ?3)
        ON CONFLICT(id) DO UPDATE SET
            current_bingo = excluded.current_bingo,
            current_bingo_starts = excluded.current_bingo_starts,
            current_bingo_ends = excluded.current_bingo_ends
        ",
        params![bingo_id, start, end],
    )
    .map(|_| ())
}

pub fn set_is_network_bingo(conn: &mut Connection, is_active: bool) -> Result<(), Error> {
    conn.execute(
        "
        INSERT INTO current_bingo_global (id, is_network_bingo)
        VALUES (1, ?1)
        ON CONFLICT(id) DO UPDATE SET
            is_network_bingo = excluded.is_network_bingo
        ",
        params![is_active],
    )
    .map(|_| ())
}

pub fn raw_batch(conn: &mut Connection, sql: String) -> Result<()> {
    conn.execute_batch(&sql)?;
    Ok(())
}
