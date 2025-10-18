use anyhow::{Context as _, Result};
use rusqlite::Connection;
use tokio::sync::oneshot;

use crate::db::{DbHandle, DbRequest};
use crate::shared::{
    db,
    types::{Bingo, BingoKind, SqlResponse},
};

pub mod read;
pub mod write;

pub fn initialise_tables(conn: &mut Connection) -> Result<(), rusqlite::Error> {
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

impl DbHandle {
    pub async fn add_bingo_mapping(&self, bingo_num: u8, bingo_kind: BingoKind) -> Result<Bingo> {
        self.dispatch_request(|response_tx| SharedDb::AddNewBingo {
            response_tx,
            bingo_id: bingo_num,
            bingo_kind,
        })
        .await?
    }

    pub async fn complete_bingo_data(&self, bingo_ids: Vec<u8>) -> Result<Vec<Bingo>> {
        self.dispatch_request(|response_tx| SharedDb::CompleteBingoData {
            response_tx,
            bingo_ids,
        })
        .await?
    }

    pub async fn update_current_bingo(
        &self,
        bingo_id: u8,
        starts_at: u32,
        ends_at: u32,
    ) -> Result<()> {
        self.dispatch_request(|response_tx| SharedDb::UpdateCurrentBingo {
            response_tx,
            bingo_id,
            starts_at,
            ends_at,
        })
        .await?
    }

    pub async fn update_is_network_bingo(&self, is_active: bool) -> Result<()> {
        self.dispatch_request(|response_tx| SharedDb::UpdateIsNetworkBingo {
            response_tx,
            is_active,
        })
        .await?
    }

    #[allow(dead_code)] // currently unused
    pub async fn get_current_bingo(&self) -> Result<Option<(u8, u32, u32)>> {
        self.dispatch_request(|response_tx| SharedDb::GetCurrentBingo { response_tx })
            .await?
    }

    pub async fn get_is_network_bingo(&self) -> Result<Option<bool>> {
        self.dispatch_request(|response_tx| SharedDb::GetIsNetworkBingo { response_tx })
            .await?
    }

    pub async fn raw_query_readonly(&self, sql: String) -> Result<SqlResponse> {
        self.dispatch_request(|response_tx| SharedDb::RawQueryReadOnly { response_tx, sql })
            .await?
    }

    #[allow(dead_code)]
    pub async fn raw_batch(&self, sql: String) -> Result<()> {
        self.dispatch_request(|response_tx| SharedDb::RawBatch { response_tx, sql })
            .await?
    }
}

pub enum SharedDb {
    AddNewBingo {
        response_tx: oneshot::Sender<Result<Bingo>>,
        bingo_id: u8,
        bingo_kind: BingoKind,
    },
    CompleteBingoData {
        response_tx: oneshot::Sender<Result<Vec<Bingo>>>,
        bingo_ids: Vec<u8>,
    },
    UpdateCurrentBingo {
        response_tx: oneshot::Sender<Result<()>>,
        bingo_id: u8,
        starts_at: u32,
        ends_at: u32,
    },
    UpdateIsNetworkBingo {
        response_tx: oneshot::Sender<Result<()>>,
        is_active: bool,
    },
    GetCurrentBingo {
        response_tx: oneshot::Sender<Result<Option<(u8, u32, u32)>>>,
    },
    GetIsNetworkBingo {
        response_tx: oneshot::Sender<Result<Option<bool>>>,
    },
    RawQueryReadOnly {
        response_tx: oneshot::Sender<Result<SqlResponse>>,
        sql: String,
    },
    RawBatch {
        response_tx: oneshot::Sender<Result<()>>,
        sql: String,
    },
}

impl DbRequest for SharedDb {
    fn execute(self: Box<Self>, conn: &mut Connection) {
        match *self {
            SharedDb::AddNewBingo {
                response_tx,
                bingo_id: bingo_num,
                bingo_kind,
            } => {
                let result = db::write::add_new_bingo(conn, bingo_num, &bingo_kind)
                    .context("Failed to insert kind-specific bingo id mapping into database");
                let _ = response_tx.send(result);
            }
            SharedDb::CompleteBingoData {
                response_tx,
                bingo_ids,
            } => {
                let result = db::read::complete_bingo_data(conn, &bingo_ids)
                    .context("Failed to fetch bingo kind mappings from database");
                let _ = response_tx.send(result);
            }
            SharedDb::UpdateCurrentBingo {
                response_tx,
                bingo_id,
                starts_at,
                ends_at,
            } => {
                let result = db::write::set_current_bingo(conn, bingo_id, starts_at, ends_at)
                    .context("Failed to update current bingo in database");
                let _ = response_tx.send(result);
            }
            SharedDb::UpdateIsNetworkBingo {
                response_tx,
                is_active,
            } => {
                let result = db::write::set_is_network_bingo(conn, is_active)
                    .context("Failed to update network bingo status is database");
                let _ = response_tx.send(result);
            }
            SharedDb::GetCurrentBingo { response_tx } => {
                let result = db::read::get_current_bingo(conn)
                    .context("Failed to fetch current bingo from database");
                let _ = response_tx.send(result);
            }
            SharedDb::GetIsNetworkBingo { response_tx } => {
                let result = db::read::get_is_network_bingo(conn)
                    .context("Failed to fetch network bingo status from database");
                let _ = response_tx.send(result);
            }
            SharedDb::RawQueryReadOnly { response_tx, sql } => {
                let result = db::read::raw_query_readonly(conn, sql);
                let _ = response_tx.send(result);
            }
            SharedDb::RawBatch { response_tx, sql } => {
                let result = db::write::raw_batch(conn, sql);
                let _ = response_tx.send(result);
            }
        }
    }
}
