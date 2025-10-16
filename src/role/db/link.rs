use rusqlite::{params, Connection, Error, OptionalExtension as _};

pub fn get_linked_user_by_discord(
    conn: &Connection,
    discord_id: u64,
) -> Result<Option<String>, Error> {
    conn.query_one(
        "SELECT minecraft_uuid FROM role_users_linked WHERE discord_id=?1",
        params![discord_id],
        |row| row.get("minecraft_uuid"),
    )
    .optional()
}

pub fn get_linked_user_by_uuid(conn: &Connection, mc_uuid: &str) -> Result<Option<u64>, Error> {
    conn.query_one(
        "SELECT discord_id FROM role_users_linked WHERE minecraft_uuid=?1",
        params![mc_uuid],
        |row| row.get("discord_id"),
    )
    .optional()
}

pub fn insert_linked_user(
    conn: &mut Connection,
    discord_id: u64,
    mc_uuid: &str,
) -> Result<(Option<u64>, Option<String>), Error> {
    let transaction = conn.transaction()?;

    let duplicate_uuid: Option<String> = {
        let mut statement = transaction
            .prepare("SELECT minecraft_uuid FROM role_users_linked WHERE discord_id=?1")?;

        statement
            .query_one(params![discord_id], |row| row.get("minecraft_uuid"))
            .optional()?
    };
    if duplicate_uuid.is_some() {
        return Ok((None, duplicate_uuid));
    }

    let duplicate_discord: Option<u64> = {
        let mut statement = transaction
            .prepare("SELECT discord_id FROM role_users_linked WHERE minecraft_uuid=?1")?;

        statement
            .query_one(params![mc_uuid], |row| row.get("discord_id"))
            .optional()?
    };
    if duplicate_discord.is_some() {
        return Ok((duplicate_discord, None));
    }

    transaction.execute(
        "
        INSERT INTO role_users_linked (discord_id, minecraft_uuid)
        VALUES (?1, ?2)
        ",
        params![discord_id, mc_uuid],
    )?;

    transaction.commit()?;

    Ok((None, None))
}

pub fn update_linked_user(
    conn: &mut Connection,
    discord_id: u64,
    mc_uuid: &str,
) -> Result<(), Error> {
    let transaction = conn.transaction()?;

    transaction.execute(
        "
        DELETE FROM role_users_linked
        WHERE discord_id=?1 OR minecraft_uuid=?2
        ",
        params![discord_id, mc_uuid],
    )?;

    transaction.execute(
        "
        INSERT INTO role_users_linked (discord_id, minecraft_uuid)
        VALUES (?1, ?2)
        ",
        params![discord_id, mc_uuid],
    )?;

    transaction.commit()?;

    Ok(())
}

pub fn remove_linked_user_by_discord(
    conn: &mut Connection,
    discord_id: u64,
) -> Result<Option<String>, Error> {
    let mut statement = conn.prepare(
        "
        DELETE FROM role_users_linked
        WHERE discord_id=?1
        RETURNING minecraft_uuid
        ",
    )?;

    statement
        .query_one(params![discord_id], |row| row.get("minecraft_uuid"))
        .optional()
}

pub fn remove_linked_user_by_uuid(
    conn: &mut Connection,
    mc_uuid: &str,
) -> Result<Option<u64>, Error> {
    let mut statement = conn.prepare(
        "
        DELETE FROM role_users_linked
        WHERE minecraft_uuid=?1
        RETURNING discord_id
        ",
    )?;

    statement
        .query_one(params![mc_uuid], |row| row.get("discord_id"))
        .optional()
}
