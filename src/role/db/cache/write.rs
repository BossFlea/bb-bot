use rusqlite::{Connection, Error, params};

use crate::shared::types::BitSet;

pub fn cache_completions(
    conn: &Connection,
    uuid: &str,
    current_bingo: u8,
    completions: &BitSet,
) -> Result<(), Error> {
    conn.execute(
        "
        INSERT OR REPLACE INTO role_completions_cache (uuid, updated_after_bingo, bingo_set)
        VALUES (?1, ?2, ?3)
        ",
        params![uuid, current_bingo, completions.data],
    )
    .map(|_| ())
}

pub fn cache_network_bingos(
    conn: &Connection,
    uuid: &str,
    current_bingo: u8,
    completions: &BitSet,
) -> Result<(), Error> {
    conn.execute(
        "
        INSERT OR REPLACE INTO role_network_bingo_cache (uuid, updated_after_bingo, bingo_set)
        VALUES (?1, ?2, ?3)
        ",
        params![uuid, current_bingo, completions.data],
    )
    .map(|_| ())
}

pub fn cache_bingo_rank(
    conn: &Connection,
    uuid: &str,
    current_bingo: u8,
    rank: u8,
) -> Result<(), Error> {
    conn.execute(
        "
        INSERT OR REPLACE INTO role_bingo_rank_cache (uuid, updated_after_bingo, rank)
        VALUES (?1, ?2, ?3)
        ",
        params![uuid, current_bingo, rank],
    )
    .map(|_| ())
}

pub fn cache_immortal(
    conn: &Connection,
    uuid: &str,
    current_bingo: u8,
    has_achieved: bool,
) -> Result<(), Error> {
    conn.execute(
        "
        INSERT OR REPLACE INTO role_immortal_cache (uuid, updated_after_bingo, has_achieved)
        VALUES (?1, ?2, ?3)
        ",
        params![uuid, current_bingo, has_achieved],
    )
    .map(|_| ())
}

pub fn cache_player_endpoint(
    conn: &Connection,
    uuid: &str,
    timestamp: i64,
    json: &str,
) -> Result<(), Error> {
    conn.execute(
        "
        INSERT OR REPLACE INTO role_player_endpoint_cache (uuid, timestamp, json)
        VALUES (?1, ?2, ?3)
        ",
        params![uuid, timestamp, json],
    )
    .map(|_| ())
}
