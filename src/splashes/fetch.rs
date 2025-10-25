use std::sync::LazyLock;

use anyhow::Result;
use poise::serenity_prelude::{
    GetMessages, Http, Mentionable as _, Message, MessageId, Timestamp, UserId,
};
use regex::Regex;

use crate::config::{SPLASH_PING_ROLE, SPLASHES_CHANNEL};

// only compile regex once per program execution
static HUB_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:dungeon|d|dung)?[\s*_~`|]*hub[^\d\n]*\d+").unwrap());

/// Helper to fetch splashes. Stores previously fetched splash messages internally on the instance
/// for future use. (for example when requesting the latest splash for a set of splashers)
pub struct FetchSplashes {
    splash_messages: Vec<Message>,
    last_id: Option<MessageId>,
    /// whether the beginning of the channel has been reached
    done: bool,
}

impl FetchSplashes {
    pub fn new() -> Self {
        Self {
            splash_messages: Vec::new(),
            last_id: None,
            done: false,
        }
    }

    /// Fetches all splashes within the specified time window, returns slice of internal vector
    pub async fn splashes_during(
        &mut self,
        http: &Http,
        start: Timestamp,
        end: Timestamp,
    ) -> Result<&[Message]> {
        if start > end {
            return Ok(&[]);
        }

        while !self.done {
            if let Some(id) = self.last_id
                && id.created_at() < start
            {
                break;
            }

            self.fetch_more(http).await?;
        }

        // first index whose value is before the end of the timeframe
        let end_index = self.splash_messages.partition_point(|m| m.timestamp > end);
        // first index whose value is before the start of the timeframe
        let start_index = self
            .splash_messages
            .partition_point(|m| m.timestamp >= start);

        Ok(&self.splash_messages[end_index..start_index])
    }

    /// Fetches splashes until one by specified user is found, then returns its timestamp (returns
    /// None if no splash is found in the last 6 months)
    pub async fn latest_splash(&mut self, http: &Http, user: UserId) -> Result<Option<Timestamp>> {
        let search_limit = Timestamp::from_unix_timestamp(
            (chrono::Utc::now() - chrono::Months::new(6)).timestamp(),
        )
        .unwrap();

        if let Some(message) = self.splash_messages.iter().find(|m| m.author.id == user) {
            return Ok(Some(message.timestamp));
        }

        while !self.done {
            if let Some(id) = self.last_id
                && id.created_at() < search_limit
            {
                break;
            }

            let batch = self.fetch_more(http).await?;

            if let Some(message) = batch.iter().find(|m| m.author.id == user) {
                return Ok(Some(message.timestamp));
            }
        }

        Ok(None)
    }

    /// Fetch an additional batch of up to 100 messages and append it to the internal vector.
    /// Returns a reference to the new batch.
    async fn fetch_more(&mut self, http: &Http) -> Result<&[Message]> {
        let mut builder = GetMessages::new().limit(100);
        if let Some(id) = self.last_id {
            builder = builder.before(id);
        }

        let batch = SPLASHES_CHANNEL.messages(http, builder).await?;

        let old_len = self.splash_messages.len();

        if let Some(message) = batch.last() {
            self.last_id = Some(message.id);
        } else {
            self.done = true;
        };

        self.splash_messages
            .extend(batch.into_iter().filter(Self::is_splash));

        if old_len < self.splash_messages.len() {
            Ok(&self.splash_messages[old_len..])
        } else {
            Ok(&[])
        }
    }

    fn is_splash(message: &Message) -> bool {
        HUB_REGEX.is_match(&message.content)
            && message
                .content
                .contains(&SPLASH_PING_ROLE.mention().to_string())
    }
}
