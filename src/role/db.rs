use rusqlite::{Connection, Result};

pub mod cache;
pub mod link;
pub mod role_config;

pub fn initialise_tables(conn: &mut Connection) -> Result<()> {
    conn.execute_batch(
        "
        -- Primarily stores patterns to attempt to auto-fetch roles if not defined explicitly,
        -- as well as role IDs if there is only one associated role (immortal)
        CREATE TABLE IF NOT EXISTS role_config_global (
            id INTEGER PRIMARY KEY CHECK (id = 1),
             -- Available placeholders:
             --   `{count}` = `5` (number of completions)
            completion_pattern TEXT DEFAULT 'Blackouts: {count}',
             -- Available placeholders:
             --   `{kind}` = `Extreme ` (empty if normal)
             --   `{number}` = `2` (kind-specific ID)
            special_completion_pattern TEXT DEFAULT '{kind}Bingo #{number} Blackout',
             -- Available placeholders:
             --   `{rank}` = `4` (bingo rank)
            bingo_rank_pattern TEXT DEFAULT 'Bingo Rank {rank}',
            immortal_pattern TEXT DEFAULT 'Immortal',
            immortal_role INTEGER
        );

        -- Create row with default values if empty
        INSERT INTO role_config_global (id)
        SELECT 1
        WHERE NOT EXISTS (SELECT 1 FROM role_config_global WHERE id = 1);

        -- Completion count -> role ID mappings (configurable, auto-populated according to pattern)
        CREATE TABLE IF NOT EXISTS role_completions_config (
            count INTEGER PRIMARY KEY,
            role INTEGER NOT NULL
        );

        -- Bingo ID -> role ID mappings (configurable, auto-populated according to pattern)
        CREATE TABLE IF NOT EXISTS role_specific_completion_config (
            kind_specific_id INTEGER NOT NULL,
            bingo_kind INTEGER NOT NULL,
            role INTEGER NOT NULL,
            PRIMARY KEY(kind_specific_id, bingo_kind)
        );

        -- Bingo rank -> role ID mappings (configurable, auto-populated according to pattern)
        CREATE TABLE IF NOT EXISTS role_bingo_rank_config (
            rank INTEGER PRIMARY KEY,
            role INTEGER NOT NULL
        );

        -- Network Bingo ID -> role ID mappings (configurable)
        CREATE TABLE IF NOT EXISTS role_network_bingo_config (
            id INTEGER PRIMARY KEY,
            role INTEGER NOT NULL
        );

        -- Linked user accounts
        CREATE TABLE IF NOT EXISTS role_users_linked (
            discord_id INTEGER PRIMARY KEY,
            minecraft_uuid TEXT NOT NULL,
            UNIQUE(minecraft_uuid)
        );

        -- Cached bingo completions
        CREATE TABLE IF NOT EXISTS role_completions_cache (
            uuid TEXT PRIMARY KEY,
            updated_after_bingo INTEGER NOT NULL,
            bingo_set BLOB
        );

        -- Cached bingo rank
        CREATE TABLE IF NOT EXISTS role_bingo_rank_cache (
            uuid TEXT PRIMARY KEY,
            updated_after_bingo INTEGER NOT NULL,
            rank INTEGER NOT NULL
        );

        -- Cached immortal status
        -- Note: immortal role is never revoked
        CREATE TABLE IF NOT EXISTS role_immortal_cache (
            uuid TEXT PRIMARY KEY,
            updated_after_bingo INTEGER NOT NULL,
            has_achieved INTEGER NOT NULL
        );

        -- Cached network bingo completions
        CREATE TABLE IF NOT EXISTS role_network_bingo_cache (
            uuid TEXT PRIMARY KEY,
            updated_after_bingo INTEGER NOT NULL,
            bingo_set BLOB
        );

        -- Cached responses from hypixel's `/v2/player` endpoint
        CREATE TABLE IF NOT EXISTS role_player_endpoint_cache (
            uuid TEXT PRIMARY KEY,
            timestamp INTEGER NOT NULL,
            json TEXT
        );
        -- Clear on startup
        DELETE FROM role_player_endpoint_cache;
        ",
    )
}
