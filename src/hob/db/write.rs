use rusqlite::{Connection, Error, Transaction, params};

use crate::hob::types::{HobEntry, OngoingSubentry};

pub fn insert_hob_entry(conn: &mut Connection, entry: &HobEntry) -> Result<(), Error> {
    let transaction = conn.transaction()?;
    match entry {
        HobEntry::OneOff {
            id,
            title,
            comment,
            bingo,
            players,
        } => {
            transaction
                .prepare("INSERT INTO hob_entries_oneoff (id, title, comment, bingo, bingo_kind) VALUES (?1, ?2, ?3, ?4, ?5)")?
                .execute(params![id, title, comment, bingo.kind_specific_id, bingo.kind as u8])?;

            insert_oneoff_players(&transaction, *id, &players.players)?;
        }
        HobEntry::Ongoing {
            id,
            title,
            comment,
            subentries,
        } => {
            transaction
                .prepare(
                    "INSERT INTO hob_entries_ongoing (id, title, comment) VALUES (?1, ?2, ?3)",
                )?
                .execute(params![id, title, comment])?;

            insert_ongoing_subentries(&transaction, *id, subentries)?;
        }
    }
    transaction.commit()?;
    Ok(())
}

pub fn insert_ongoing_subentry(
    conn: &mut Connection,
    subentry: &OngoingSubentry,
    entry_id: u64,
) -> Result<(), Error> {
    conn.execute(
        "
        INSERT INTO hob_ongoing_subentries (id, entry_id, player, value, bingo, bingo_kind)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ",
        params![
            subentry.id,
            entry_id,
            subentry.player,
            subentry.value,
            subentry.bingo.kind_specific_id,
            subentry.bingo.kind as u8
        ],
    )?;

    Ok(())
}

pub fn update_hob_entry(conn: &mut Connection, entry: &HobEntry) -> Result<(), Error> {
    let transaction = conn.transaction()?;
    match entry {
        HobEntry::OneOff {
            id,
            title,
            comment,
            bingo,
            players,
        } => {
            transaction
                .prepare(
                    "
                    INSERT INTO hob_entries_oneoff (id, title, comment, bingo, bingo_kind)
                    VALUES (?1, ?2, ?3, ?4, ?5)
                    ON CONFLICT(id) DO UPDATE SET
                        title = excluded.title,
                        comment = excluded.comment,
                        bingo = excluded.bingo,
                        bingo_kind = excluded.bingo_kind
                    ",
                )?
                .execute(params![
                    id,
                    title,
                    comment,
                    bingo.kind_specific_id,
                    bingo.kind as u8
                ])?;

            transaction
                .prepare("DELETE FROM hob_oneoff_players WHERE entry_id=?1")?
                .execute(params![id])?;

            insert_oneoff_players(&transaction, *id, &players.players)?;
        }
        HobEntry::Ongoing {
            id, title, comment, ..
        } => {
            transaction
                .prepare(
                    "
                    INSERT INTO hob_entries_ongoing (id, title, comment)
                    VALUES (?1, ?2, ?3)
                    ON CONFLICT(id) DO UPDATE SET
                        title = excluded.title,
                        comment = excluded.comment
                    ",
                )?
                .execute(params![id, title, comment])?;
        }
    }
    transaction.commit()?;
    Ok(())
}

pub fn update_ongoing_subentry(
    conn: &mut Connection,
    subentry: &OngoingSubentry,
) -> Result<(), Error> {
    conn.prepare(
        "
        INSERT INTO hob_ongoing_subentries (id, entry_id, player, value, bingo, bingo_kind)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(id) DO UPDATE SET
            player = excluded.player,
            value = excluded.value,
            bingo = excluded.bingo,
            bingo_kind = excluded.bingo_kind
        ",
    )?
    .execute(params![
        subentry.id,
        subentry.entry_id,
        subentry.player,
        subentry.value,
        subentry.bingo.kind_specific_id,
        subentry.bingo.kind as u8
    ])?;

    Ok(())
}

fn insert_oneoff_players(
    transaction: &Transaction,
    entry_id: u64,
    players: &[String],
) -> Result<(), Error> {
    let mut player_statement = transaction.prepare(
        "INSERT INTO hob_oneoff_players (entry_id, player, position) VALUES (?1, ?2, ?3)",
    )?;

    for (i, player) in players.iter().enumerate() {
        player_statement.execute(params![entry_id, player, i])?;
    }

    Ok(())
}

fn insert_ongoing_subentries(
    transaction: &Transaction,
    entry_id: u64,
    subentries: &[OngoingSubentry],
) -> Result<(), Error> {
    let mut subentry_statement = transaction.prepare(
        "INSERT INTO hob_ongoing_subentries (id, entry_id, player, value, bingo, bingo_kind) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;

    for subentry in subentries {
        subentry_statement.execute(params![
            subentry.id,
            entry_id,
            subentry.player,
            subentry.value,
            subentry.bingo.kind_specific_id,
            subentry.bingo.kind as u8
        ])?;
    }

    Ok(())
}

pub fn delete_hob_entry_by_id(conn: &mut Connection, id: u64) -> Result<(), Error> {
    let transaction = conn.transaction()?;
    let oneoff_entries_deleted = {
        let mut statement = transaction.prepare("DELETE FROM hob_entries_oneoff WHERE id=?1")?;
        statement.execute([id])?
    };

    if oneoff_entries_deleted > 0 {
        transaction.commit()?;
        return Ok(());
    }

    {
        let mut statement = transaction.prepare("DELETE FROM hob_entries_ongoing WHERE id=?1")?;
        statement.execute([id])?;
    }

    transaction.commit()?;
    Ok(())
}

pub fn delete_ongoing_subentry_by_id(
    conn: &mut Connection,
    id: u64,
    entry_id: u64,
) -> Result<(), Error> {
    let mut statement =
        conn.prepare("DELETE FROM hob_ongoing_subentries WHERE id=?1 AND entry_id=?2")?;
    statement.execute([id, entry_id])?;

    Ok(())
}
