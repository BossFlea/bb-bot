use anyhow::{anyhow, bail, Context as _, Result};
use reqwest::Client;
use serde_json::Value;

use crate::error::UserError;

pub async fn uuid(client: &Client, username: &str) -> Result<String> {
    let username = username.trim();
    if !validate_mc_username(username) {
        bail!(UserError(anyhow!("Invalid Minecraft username: {username}")));
    }

    let response = client
        .get(format!(
            "https://api.minecraftservices.com/minecraft/profile/lookup/name/{username}"
        ))
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;
    let json: Value = serde_json::from_str(&text)?;

    if !status.is_success() {
        bail!(UserError(anyhow!(
            "Failed to fetch UUID for username: {}",
            json["errorMessage"].as_str().unwrap_or_default()
        )));
    }

    let uuid = json["id"]
        .as_str()
        .context("No UUID in response")?
        .replace("-", "");

    Ok(uuid)
}

pub async fn username(client: &Client, uuid: &str) -> Result<String> {
    let response = client
        .get(format!(
            "https://api.minecraftservices.com/minecraft/profile/lookup/{uuid}"
        ))
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;
    let json: Value = serde_json::from_str(&text)?;

    if !status.is_success() {
        bail!(
            "Failed to fetch username: {}",
            json["errorMessage"].as_str().unwrap_or_default()
        );
    }

    let uuid = json["name"]
        .as_str()
        .context("No username in response")?
        .to_string();

    Ok(uuid)
}

fn validate_mc_username(username: &str) -> bool {
    username.len() <= 16
        && username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
}
