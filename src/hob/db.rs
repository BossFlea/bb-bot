use rusqlite::{Connection, Result};

mod read;
mod write;

pub use read::*;
pub use write::*;

pub fn initialise_tables(conn: &mut Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS hob_entries_oneoff (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            comment TEXT,
            bingo INTEGER NOT NULL,
            bingo_kind INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS hob_oneoff_players (
            entry_id INTEGER NOT NULL,
            player TEXT NOT NULL,
            position INTEGER NOT NULL,
            PRIMARY KEY(entry_id, player),
            FOREIGN KEY(entry_id) REFERENCES hob_entries_oneoff(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS hob_entries_ongoing (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            comment TEXT
        );
        CREATE TABLE IF NOT EXISTS hob_ongoing_subentries (
            id INTEGER PRIMARY KEY,
            entry_id INTEGER NOT NULL,
            player TEXT NOT NULL,
            value TEXT NOT NULL,
            bingo INTEGER NOT NULL,
            bingo_kind INTEGER NOT NULL,
            FOREIGN KEY(entry_id) REFERENCES hob_entries_ongoing(id) ON DELETE CASCADE
        );
        ",
    )
}
