use std::thread;

use anyhow::{Context, Result};
use rusqlite::Connection;
use tokio::sync::{mpsc, oneshot};

use crate::config::DB_PATH;
use crate::db::DbRequest;

pub fn start_db_thread(
    mut rx: mpsc::Receiver<Box<dyn DbRequest>>,
) -> oneshot::Receiver<Result<()>> {
    let (ready_tx, ready_rx) = oneshot::channel();

    thread::spawn(move || {
        let mut conn = match initialise_database() {
            Ok(conn) => conn,
            Err(err) => {
                let _ = ready_tx.send(Err(err.context("Failed to initialise database")));
                return;
            }
        };

        let _ = ready_tx.send(Ok(()));

        while let Some(request) = rx.blocking_recv() {
            request.execute(&mut conn);
        }
    });

    ready_rx
}

fn initialise_database() -> Result<Connection> {
    let mut conn = Connection::open(DB_PATH).context("Unable to load database file")?;

    conn.pragma_update(None, "foreign_keys", true)
        .context("Failed to configure database")?;

    crate::hob::db::initialise_tables(&mut conn)?;
    crate::role::db::initialise_tables(&mut conn)?;
    crate::shared::db::initialise_tables(&mut conn)?;
    Ok(conn)
}
