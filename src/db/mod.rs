use anyhow::Result;
use rusqlite::Connection;
use tokio::sync::{mpsc, oneshot};

pub mod db_thread;

pub trait DbRequest: Send + Sync + 'static {
    type ReturnValue: Send + 'static;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue;
}

pub struct DbHandle {
    tx: mpsc::Sender<Box<dyn ErasedDbRequest>>,
}

impl DbHandle {
    pub fn new(tx: mpsc::Sender<Box<dyn ErasedDbRequest>>) -> Self {
        Self { tx }
    }

    pub async fn request<R>(&self, req: R) -> Result<R::ReturnValue>
    where
        R: DbRequest,
    {
        let (resp_tx, resp_rx) = oneshot::channel();
        let wrapped = RequestWrapper {
            inner: req,
            resp_tx,
        };
        self.tx.send(Box::new(wrapped)).await?;
        Ok(resp_rx.await?)
    }
}

pub trait ErasedDbRequest: Send + Sync {
    fn execute_boxed(self: Box<Self>, conn: &mut Connection);
}

struct RequestWrapper<R: DbRequest> {
    inner: R,
    resp_tx: oneshot::Sender<R::ReturnValue>,
}

impl<R: DbRequest> ErasedDbRequest for RequestWrapper<R> {
    fn execute_boxed(self: Box<Self>, conn: &mut Connection) {
        let result = self.inner.execute(conn);
        let _ = self.resp_tx.send(result);
    }
}
