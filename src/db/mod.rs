use anyhow::Result;
use rusqlite::Connection;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

pub mod db_thread;

pub trait DbRequest: Send + Sync + 'static {
    fn execute(self: Box<Self>, conn: &mut Connection);
}

pub struct DbHandle {
    pub db_tx: Sender<Box<dyn DbRequest>>,
}

impl DbHandle {
    pub async fn dispatch_request<R, T>(
        &self,
        request: impl FnOnce(oneshot::Sender<T>) -> R,
    ) -> Result<T>
    where
        R: DbRequest,
    {
        let (response_tx, response_rx) = oneshot::channel();
        self.db_tx.send(Box::new(request(response_tx))).await?;
        Ok(response_rx.await?)
    }
}
