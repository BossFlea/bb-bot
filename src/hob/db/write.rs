use rusqlite::{Connection, Result, Transaction, params};

use crate::db::DbRequest;
use crate::hob::types::{HobEntry, OngoingSubentry};

pub struct InsertHobEntry {
    pub entry: HobEntry,
}
impl DbRequest for InsertHobEntry {
    type ReturnValue = Result<()>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let transaction = conn.transaction()?;
        match self.entry {
            HobEntry::OneOff {
                id,
                title,
                comment,
                bingo,
                players,
            } => {
                transaction.execute(
                    "
                    INSERT INTO hob_entries_oneoff (id, title, comment, bingo, bingo_kind)
                    VALUES (?1, ?2, ?3, ?4, ?5)
                    ",
                    params![id, title, comment, bingo.kind_specific_id, bingo.kind as u8],
                )?;

                insert_oneoff_players(&transaction, id, &players.players)?;
            }
            HobEntry::Ongoing {
                id,
                title,
                comment,
                subentries,
            } => {
                transaction.execute(
                    "
                    INSERT INTO hob_entries_ongoing (id, title, comment)
                    VALUES (?1, ?2, ?3)
                    ",
                    params![id, title, comment],
                )?;

                insert_ongoing_subentries(&transaction, id, &subentries)?;
            }
        }
        transaction.commit()?;
        Ok(())
    }
}

pub struct InsertHobSubentry {
    pub subentry: OngoingSubentry,
    pub ongoing_entry_id: u64,
}
impl DbRequest for InsertHobSubentry {
    type ReturnValue = Result<()>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        conn.execute(
            "
            INSERT INTO hob_ongoing_subentries (id, entry_id, player, value, bingo, bingo_kind)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                self.subentry.id,
                self.ongoing_entry_id,
                self.subentry.player,
                self.subentry.value,
                self.subentry.bingo.kind_specific_id,
                self.subentry.bingo.kind as u8
            ],
        )?;

        Ok(())
    }
}

pub struct UpdateHobEntry {
    pub entry: HobEntry,
}
impl DbRequest for UpdateHobEntry {
    type ReturnValue = Result<()>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let transaction = conn.transaction()?;
        match self.entry {
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

                insert_oneoff_players(&transaction, id, &players.players)?;
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
}

pub struct UpdateHobSubentry {
    pub subentry: OngoingSubentry,
}
impl DbRequest for UpdateHobSubentry {
    type ReturnValue = Result<()>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
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
            self.subentry.id,
            self.subentry.entry_id,
            self.subentry.player,
            self.subentry.value,
            self.subentry.bingo.kind_specific_id,
            self.subentry.bingo.kind as u8
        ])?;

        Ok(())
    }
}

fn insert_oneoff_players(
    transaction: &Transaction,
    entry_id: u64,
    players: &[String],
) -> Result<()> {
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
) -> Result<()> {
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

pub struct DeleteHobEntry {
    pub id: u64,
}
impl DbRequest for DeleteHobEntry {
    type ReturnValue = Result<()>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let transaction = conn.transaction()?;
        let oneoff_entries_deleted = {
            let mut statement =
                transaction.prepare("DELETE FROM hob_entries_oneoff WHERE id=?1")?;
            statement.execute([self.id])?
        };

        if oneoff_entries_deleted > 0 {
            transaction.commit()?;
            return Ok(());
        }

        {
            let mut statement =
                transaction.prepare("DELETE FROM hob_entries_ongoing WHERE id=?1")?;
            statement.execute([self.id])?;
        }

        transaction.commit()?;
        Ok(())
    }
}

pub struct DeleteHobSubentry {
    pub id: u64,
    pub entry_id: u64,
}
impl DbRequest for DeleteHobSubentry {
    type ReturnValue = Result<()>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let mut statement =
            conn.prepare("DELETE FROM hob_ongoing_subentries WHERE id=?1 AND entry_id=?2")?;
        statement.execute([self.id, self.entry_id])?;

        Ok(())
    }
}
