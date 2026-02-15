use std::str::FromStr as _;

use anyhow::{Context as _, Result, bail};
use chrono::{DateTime, Datelike as _};
use serde_json::Value;
use tracing::warn;

use crate::db::DbHandle;
use crate::hypixel_api::ApiHandle;
use crate::role::{
    db::cache::{CacheHypixelPlayerEndpoint, CachedHypixelPlayerEndpoint},
    types::NetworkBingo,
};
use crate::shared::{
    db::{AddBingoMapping, SetCurrentBingo},
    types::{Bingo, BingoKind},
};

pub mod network_bingo;

pub async fn query_api(
    handle: &ApiHandle,
    endpoint: &str,
    params: &[(&str, &str)],
) -> Result<(Value, String)> {
    let params: String = params
        .iter()
        .map(|p| format!("&{}={}", p.0, p.1))
        .collect::<Vec<_>>()
        .join("");

    let response = handle
        .client
        .get(format!(
            "https://api.hypixel.net{endpoint}?key={}{}",
            handle.api_key, params,
        ))
        .send()
        .await?;
    let text = response.text().await?;
    let json: Value = serde_json::from_str(&text)?;

    if !json["success"]
        .as_bool()
        .context("Unsuccessful Hypixel API request: No success indicator received")?
    {
        if let Some(cause) = json["cause"].as_str() {
            bail!("Unsuccessful Hypixel API request: {cause}")
        }
        bail!("Unsuccessful Hypixel API request");
    };

    Ok((json, text))
}

pub async fn get_current_bingo_data(
    handle: &ApiHandle,
    db: &DbHandle,
) -> Result<(Bingo, i64, i64)> {
    let (json, _) = query_api(handle, "/v2/resources/skyblock/bingo", &[]).await?;

    let bingo_id = json["id"]
        .as_u64()
        .context("No bingo ID found for current bingo")? as u8;

    let bingo_kind = match json["modifier"]
        .as_str()
        .context("No bingo type found for current bingo")?
    {
        "EXTREME" => BingoKind::Extreme,
        "SECRET" => BingoKind::Secret,
        _ => BingoKind::Normal,
    };

    let start = json["start"]
        .as_i64()
        .context("No start time found for current bingo")?
        / 1000;

    let end = json["end"]
        .as_i64()
        .context("No end time found for current bingo")?
        / 1000;

    db.request(SetCurrentBingo {
        bingo_id,
        start,
        end,
    })
    .await??;
    let bingo = db
        .request(AddBingoMapping {
            bingo_id,
            bingo_kind,
        })
        .await??;

    Ok((bingo, start, end))
}

pub async fn linked_discord(
    handle: &ApiHandle,
    db: &DbHandle,
    uuid: &str,
) -> Result<Option<String>> {
    let params = [("uuid", uuid)];
    // only fetch from API if no valid cached result exists
    // NOTE: this is necessary because of an undocumented 60s rate limit on fetching the same
    // player, which applies *only* to application keys, not dev keys
    let json = match db
        .request(CachedHypixelPlayerEndpoint {
            uuid: uuid.to_string(),
        })
        .await??
    {
        Some((_, json_str)) => Value::from_str(&json_str)?,
        None => {
            let (json, raw_json) = query_api(handle, "/v2/player", &params).await?;
            db.request(CacheHypixelPlayerEndpoint {
                uuid: uuid.to_string(),
                timestamp: chrono::Utc::now().timestamp(),
                json: raw_json,
            })
            .await??;
            json
        }
    };

    let discord = json["player"]["socialMedia"]["links"]["DISCORD"]
        .as_str()
        .map(String::from);

    Ok(discord)
}

pub async fn bingo_completions(handle: &ApiHandle, uuid: &str) -> Result<Vec<u8>> {
    let params = [("uuid", uuid)];
    // NOTE: This errors if the user has never touched bingo
    let (json, _) = match query_api(handle, "/v2/skyblock/bingo", &params).await {
        Ok(response) => response,
        Err(err) => {
            warn!("No Bingo data for '{uuid}': {err}");
            return Ok(Vec::new());
        }
    };

    let completed_goals: Vec<u8> = json["events"].as_array().map_or_else(Vec::new, |events| {
        events
            .iter()
            .filter_map(|bingo| {
                bingo["completed_goals"]
                    .as_array()
                    .map(|goals| goals.len() == 20)
                    .unwrap_or(false)
                    .then(|| bingo["key"].as_u64().and_then(|n| n.try_into().ok()))
                    .flatten()
            })
            .collect()
    });

    Ok(completed_goals)
}

#[derive(Debug)]
pub struct BingoProfileData {
    pub created_during: u8,
    pub bingo_rank: u8,
    pub has_deaths: bool,
}

pub async fn bingo_profile_data(
    handle: &ApiHandle,
    uuid: &str,
) -> Result<Option<BingoProfileData>> {
    let params = [("uuid", uuid)];
    let (profiles_json, _) = query_api(handle, "/v2/skyblock/profiles", &params).await?;

    let profile_id = profiles_json["profiles"].as_array().and_then(|profiles| {
        profiles.iter().find_map(|p| {
            (p["game_mode"].as_str()? == "bingo")
                .then(|| p["profile_id"].as_str())
                .flatten()
        })
    });

    let profile_id = match profile_id {
        Some(id) => id,
        None => return Ok(None),
    };

    let params = [("profile", profile_id)];
    let (json, _) = query_api(handle, "/v2/skyblock/profile", &params).await?;

    let has_deaths: bool = json["profile"]["members"][uuid]["player_stats"]["deaths"]
        .as_object()
        .map(|d| {
            d.keys().any(|k| {
                !ALLOWED_DEATH_CAUSES.iter().any(|cause| {
                    k == cause || (k.starts_with("master_") && &k["master_".len()..] == *cause)
                })
            })
        })
        .unwrap_or(false);

    let bingo_rank: u8 = json["profile"]["members"][uuid]["pets_data"]["pets"]
        .as_array()
        .and_then(|pets| {
            pets.iter().find_map(|pet| {
                (pet["type"].as_str()? == "BINGO")
                    .then(|| {
                        Some(match pet["tier"].as_str()? {
                            "MYTHIC" => 5,
                            "LEGENDARY" => 4,
                            "EPIC" => 3,
                            "RARE" => 2,
                            "UNCOMMON" => 1,
                            _ => 0,
                        })
                    })
                    .flatten()
            })
        })
        .unwrap_or(0);

    let created_during = bingo_id_from_timestamp(
        json["profile"]["created_at"]
            .as_u64()
            .map_or(0, |t| (t / 1000) as u32),
    )?;

    Ok(Some(BingoProfileData {
        created_during,
        bingo_rank,
        has_deaths,
    }))
}

// NOTE: Calculations are deliberately done in UTC, because this accounts for the weird 1h bingo
// time shift that Hypixel sometimes has around DST by using the ~4-5h UTC-EST difference as a
// buffer zone, while keeping timezone calculations simple as a side effect
pub fn bingo_id_from_timestamp(timestamp: u32) -> Result<u8> {
    const FIRST_BINGO_INDEX: u32 = 2021 * 12 + 12;

    let time_utc = DateTime::from_timestamp(timestamp.into(), 0).context("Invalid timestamp")?;

    let year = time_utc.year() as u32;
    let month = time_utc.month();

    let timestamp_index = year * 12 + month;

    Ok(timestamp_index.saturating_sub(FIRST_BINGO_INDEX) as u8)
}

pub async fn network_bingo_completions(
    handle: &ApiHandle,
    db: &DbHandle,
    uuid: &str,
) -> Result<Vec<NetworkBingo>> {
    let params = [("uuid", uuid)];
    // only fetch from API if no valid cached result exists
    let json = match db
        .request(CachedHypixelPlayerEndpoint {
            uuid: uuid.to_string(),
        })
        .await??
    {
        Some((_, json_str)) => Value::from_str(&json_str)?,
        None => {
            let (json, raw_json) = query_api(handle, "/v2/player", &params).await?;
            db.request(CacheHypixelPlayerEndpoint {
                uuid: uuid.to_string(),
                timestamp: chrono::Utc::now().timestamp(),
                json: raw_json,
            })
            .await??;
            json
        }
    };

    let seasonal_events = network_bingo::network_bingo_completions(&json["player"]["seasonal"]);

    Ok(seasonal_events)
}

const ALLOWED_DEATH_CAUSES: &[&str] = &[
    "total", // total death count entry
    "trap",
    "diamond_guy",
    "cellar_spider",
    "crypt_dreadlord",
    "crypt_lurker",
    "crypt_souleater",
    "lonely_spider",
    "lost_adventurer",
    "scared_skeleton",
    "skeleton_grunt",
    "sniper_skeleton",
    "watcher",
    "dungeon_respawning_skeleton",
    "zombie_grunt",
    "watcher_summon_undead",
    "crypt_undead",
    "skeleton_soldier",
    "bonzo",
    "bonzo_summon_undead",
    "watcher_bonzo",
    "skeleton_master",
    "scarf",
    "scarf_archer",
    "scarf_mage",
    "scarf_warrior",
    "watcher_scarf",
    "deathmite",
    "king_midas",
    "shadow_assassin",
    "skeletor",
    "zombie_knight",
    "professor",
    "professor_guardian_summon",
    "professor_mage_guardian",
    "professor_archer_guardian",
    "professor_warrior_guardian",
    "professor_priest_guardian",
    "super_tank_zombie",
    "zombie_soldier",
    "spirit_miniboss",
    "spirit_bat",
    "spirit_bull",
    "spirit_chicken",
    "spirit_rabbit",
    "spirit_sheep",
    "spirit_wolf",
    "livid",
    "livid_clone",
    "watcher_livid",
    "tentaclees",
    "mimic",
    "skeletor_prime",
    "super_archer",
    "zombie_commander",
    "crypt_witherskeleton",
    "sadan_giant",
    "sadan_golem",
    "sadan_statue",
    "skeleton_lord",
    "wither_guard",
    "wither_miner",
    "zombie_lord",
    "maxor",
    "storm",
    "goldor",
    "necron",
];
