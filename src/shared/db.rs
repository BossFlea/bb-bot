use rusqlite::{Connection, Result};

mod read;
mod write;

pub use read::*;
pub use write::*;

pub fn initialise_tables(conn: &mut Connection) -> Result<()> {
    conn.execute_batch(
        "
        -- Stores data about the current bingo
        CREATE TABLE IF NOT EXISTS current_bingo_global (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            current_bingo INTEGER,
            current_bingo_starts INTEGER,
            current_bingo_ends INTEGER,
            is_network_bingo INTEGER
        );

        -- Maps unique bingo IDs to specific bingo kind and the kind-specific ID (e.g. unique ID 21 -> Extreme with specific ID 2)
        CREATE TABLE IF NOT EXISTS bingo_kind_id_map (
            bingo INTEGER PRIMARY KEY,
            bingo_kind INTEGER NOT NULL,
            kind_specific_id INTEGER NOT NULL,
            UNIQUE(bingo_kind, kind_specific_id)
        );
        ",
    )
}
