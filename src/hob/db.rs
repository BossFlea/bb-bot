use anyhow::{Context as _, Result};
use rusqlite::Connection;
use tokio::sync::oneshot::{self};

use crate::db::{DbRequest, DbHandle};
use crate::hob::{
    db,
    types::{HobEntry, OngoingSubentry},
};

pub mod read;
pub mod write;

pub fn initialise_tables(conn: &mut Connection) -> Result<(), rusqlite::Error> {
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

impl DbHandle {
    pub async fn insert_hob_entry(&self, entry: HobEntry) -> Result<()> {
        self.dispatch_request(|response_tx| HobDb::InsertHobEntry { response_tx, entry })
            .await?
    }

    pub async fn insert_ongoing_subentry(
        &self,
        subentry: OngoingSubentry,
        entry_id: u64,
    ) -> Result<()> {
        self.dispatch_request(|response_tx| HobDb::InsertOngoingSubentry {
            response_tx,
            subentry,
            entry_id,
        })
        .await?
    }

    pub async fn get_all_hob_entries(&self) -> Result<Vec<HobEntry>> {
        self.dispatch_request(|response_tx| HobDb::GetAllHobEntries { response_tx })
            .await?
    }

    pub async fn get_hob_entry_by_id(&self, id: u64) -> Result<Option<HobEntry>> {
        self.dispatch_request(|response_tx| HobDb::GetHobEntryById { response_tx, id })
            .await?
    }

    pub async fn get_ongoing_subentry_by_id(
        &self,
        id: u64,
        entry_id: u64,
    ) -> Result<Option<OngoingSubentry>> {
        self.dispatch_request(|response_tx| HobDb::GetOngoingSubentryById {
            response_tx,
            id,
            entry_id,
        })
        .await?
    }

    pub async fn delete_hob_entry_by_id(&self, id: u64) -> Result<()> {
        self.dispatch_request(|response_tx| HobDb::DeleteHobEntryById { response_tx, id })
            .await?
    }

    pub async fn delete_ongoing_subentry_by_id(&self, id: u64, entry_id: u64) -> Result<()> {
        self.dispatch_request(|response_tx| HobDb::DeleteOngoingSubentryById {
            response_tx,
            id,
            entry_id,
        })
        .await?
    }

    pub async fn search_entries_with_content(&self, query: String) -> Result<Vec<HobEntry>> {
        self.dispatch_request(|response_tx| HobDb::SearchEntriesWithContent { response_tx, query })
            .await?
    }

    pub async fn update_hob_entry(&self, data: HobEntry) -> Result<()> {
        self.dispatch_request(|response_tx| HobDb::UpdateEntryData { response_tx, data })
            .await?
    }

    pub async fn update_ongoing_subentry(&self, data: OngoingSubentry) -> Result<()> {
        self.dispatch_request(|response_tx| HobDb::UpdateOngoingSubentryData { response_tx, data })
            .await?
    }
}

pub enum HobDb {
    InsertHobEntry {
        response_tx: oneshot::Sender<Result<()>>,
        entry: HobEntry,
    },
    InsertOngoingSubentry {
        response_tx: oneshot::Sender<Result<()>>,
        subentry: OngoingSubentry,
        entry_id: u64,
    },
    GetAllHobEntries {
        response_tx: oneshot::Sender<Result<Vec<HobEntry>>>,
    },
    GetHobEntryById {
        response_tx: oneshot::Sender<Result<Option<HobEntry>>>,
        id: u64,
    },
    GetOngoingSubentryById {
        response_tx: oneshot::Sender<Result<Option<OngoingSubentry>>>,
        id: u64,
        entry_id: u64,
    },
    DeleteHobEntryById {
        response_tx: oneshot::Sender<Result<()>>,
        id: u64,
    },
    DeleteOngoingSubentryById {
        response_tx: oneshot::Sender<Result<()>>,
        id: u64,
        entry_id: u64,
    },
    SearchEntriesWithContent {
        response_tx: oneshot::Sender<Result<Vec<HobEntry>>>,
        query: String,
    },
    UpdateEntryData {
        response_tx: oneshot::Sender<Result<()>>,
        data: HobEntry,
    },
    UpdateOngoingSubentryData {
        response_tx: oneshot::Sender<Result<()>>,
        data: OngoingSubentry,
    },
}

impl DbRequest for HobDb {
    fn execute(self: Box<Self>, conn: &mut Connection) {
        match *self {
            HobDb::InsertHobEntry { response_tx, entry } => {
                let result = db::write::insert_hob_entry(conn, &entry)
                    .context("Failed to insert HoB entry into database");
                let _ = response_tx.send(result);
            }
            HobDb::InsertOngoingSubentry {
                response_tx,
                subentry,
                entry_id,
            } => {
                let result = db::write::insert_ongoing_subentry(conn, &subentry, entry_id)
                    .context("Failed to insert subentry into database");
                let _ = response_tx.send(result);
            }
            HobDb::GetAllHobEntries { response_tx } => {
                let result = db::read::get_all_hob_entries(conn)
                    .context("Failed to fetch HoB entries from database");
                let _ = response_tx.send(result);
            }
            HobDb::GetHobEntryById { response_tx, id } => {
                let result = db::read::get_hob_entry_by_id(conn, id)
                    .context("Failed to fetch HoB entry from database");
                let _ = response_tx.send(result);
            }
            HobDb::GetOngoingSubentryById {
                response_tx,
                id,
                entry_id,
            } => {
                let result = db::read::get_ongoing_subentry_by_id(conn, id, entry_id)
                    .context("Failed to fetch Ongoing HoB sub-entry from database");
                let _ = response_tx.send(result);
            }
            HobDb::DeleteHobEntryById { response_tx, id } => {
                let result = db::write::delete_hob_entry_by_id(conn, id)
                    .context("Failed to delete HoB entry from database");
                let _ = response_tx.send(result);
            }
            HobDb::DeleteOngoingSubentryById {
                response_tx,
                id,
                entry_id,
            } => {
                let result = db::write::delete_ongoing_subentry_by_id(conn, id, entry_id)
                    .context("Failed to delete subentry from database");
                let _ = response_tx.send(result);
            }
            HobDb::SearchEntriesWithContent { response_tx, query } => {
                let result = db::read::search_entries_with_content(conn, &query)
                    .context("Failed to search database for entries with query");
                let _ = response_tx.send(result);
            }
            HobDb::UpdateEntryData { response_tx, data } => {
                let result = db::write::update_hob_entry(conn, &data)
                    .context("Failed to update entry in database");
                let _ = response_tx.send(result);
            }
            HobDb::UpdateOngoingSubentryData { response_tx, data } => {
                let result = db::write::update_ongoing_subentry(conn, &data)
                    .context("Failed to update subentry in database");
                let _ = response_tx.send(result);
            }
        }
    }
}
