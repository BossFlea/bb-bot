use std::collections::HashMap;

use anyhow::Result;
use poise::serenity_prelude::{Http, Timestamp, UserId};

use crate::splashes::fetch::FetchSplashes;

pub async fn latest_splash(http: &Http, user: UserId) -> Result<Option<Timestamp>> {
    let mut splashes = FetchSplashes::new();

    splashes.latest_splash(http, user).await
}

pub async fn latest_splash_batch(
    http: &Http,
    users: &[UserId],
) -> Result<HashMap<UserId, Timestamp>> {
    let mut splashes = FetchSplashes::new();

    let mut users_map = HashMap::new();

    for user in users {
        if let Some(timestamp) = splashes.latest_splash(http, *user).await? {
            users_map.insert(*user, timestamp);
        }
    }

    Ok(users_map)
}
