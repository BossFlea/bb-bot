use anyhow::{Context as _, Result};
use reqwest::Client;

use crate::db::DbHandle;
use crate::hypixel_api::hypixel::BingoProfileData;
use crate::role::types::NetworkBingo;
use crate::shared::types::Bingo;

mod hypixel;
mod mojang;

pub struct ApiHandle {
    client: Client,
    api_key: String,
}

impl ApiHandle {
    pub fn new(key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: key,
        }
    }

    const INVALID_RESPONSE: &str = "Invalid response from Hypixel's API";

    pub async fn uuid(&self, username: &str) -> Result<String> {
        mojang::uuid(&self.client, username).await
    }

    pub async fn username(&self, uuid: &str) -> Result<String> {
        mojang::username(&self.client, uuid).await
    }

    pub async fn linked_discord(&self, uuid: &str) -> Result<Option<String>> {
        hypixel::linked_discord(self, uuid)
            .await
            .context(Self::INVALID_RESPONSE)
    }

    pub async fn update_current_bingo(&self, db: &DbHandle) -> Result<(Bingo, bool)> {
        hypixel::get_current_bingo_data(self, db)
            .await
            .context(Self::INVALID_RESPONSE)
    }

    pub async fn bingo_completions(&self, uuid: &str) -> Result<Vec<u8>> {
        hypixel::bingo_completions(self, uuid)
            .await
            .context(Self::INVALID_RESPONSE)
    }

    pub async fn bingo_profile_data(&self, uuid: &str) -> Result<Option<BingoProfileData>> {
        hypixel::bingo_profile_data(self, uuid)
            .await
            .context(Self::INVALID_RESPONSE)
    }

    pub async fn network_bingo_completions(&self, uuid: &str) -> Result<Vec<NetworkBingo>> {
        hypixel::network_bingo_completions(self, uuid)
            .await
            .context(Self::INVALID_RESPONSE)
    }
}
