use std::sync::Arc;

use poise::serenity_prelude::{Http, MessageId};
use tokio::sync::oneshot;

pub mod event;
mod reminder;

pub struct SplashReminderHandle {
    latest: Option<MessageId>,
    cancel_tx: Option<oneshot::Sender<()>>,
}

impl SplashReminderHandle {
    pub fn new() -> Self {
        Self {
            latest: None,
            cancel_tx: None,
        }
    }

    pub fn latest(&self) -> Option<MessageId> {
        self.latest
    }

    fn cancel_timer(&mut self) {
        if let Some(tx) = self.cancel_tx.take() {
            // this `Result` is irrelevant, as failure simply means the timer has already run out
            // and dropped the receiver
            let _ = tx.send(());
        }
    }

    pub fn clear_latest(&mut self) {
        self.cancel_timer();
        self.latest = None;
    }

    pub async fn new_splash(&mut self, http: Arc<Http>, message: MessageId) {
        self.latest = Some(message);

        // cancel previous timer if present
        self.cancel_timer();

        // initiate new timer
        let (cancel_tx, cancel_rx) = oneshot::channel();

        self.cancel_tx = Some(cancel_tx);

        reminder::spawn_timer(http, cancel_rx).await;
    }
}
