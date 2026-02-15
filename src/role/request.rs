use std::sync::Arc;

use anyhow::{Context as _, Result};
use chrono::Utc;
use poise::serenity_prelude::{
    ButtonStyle, CacheHttp as _, Context as SerenityContext, CreateButton, CreateComponent,
    CreateContainer, CreateContainerComponent, CreateSection, CreateSectionAccessory,
    CreateSectionComponent, CreateTextDisplay, Member, Mentionable as _, RoleId, User, UserId,
    colours::{branding::YELLOW, css::POSITIVE},
};
use tracing::warn;

use crate::config::BOT_MAINTAINER;
use crate::role::{
    db::{
        cache::{
            CacheBingoRank, CacheCompletions, CacheImmortal, CacheNetworkBingos, CachedBingoRank,
            CachedCompletions, CachedImmortal, CachedNetworkBingos,
        },
        link::InsertLinkedUser,
        role_config::{
            BuildRoleDeltaBingoRank, BuildRoleDeltaCompletions, BuildRoleDeltaImmortal,
            BuildRoleDeltaNetworkBingos,
        },
    },
    types::{LinkStatus, LinkedUser, NetworkBingo},
};
use crate::shared::{
    BotData,
    db::{GetBingoData, GetIsNetworkBingo},
    types::{Bingo, BitSet},
};

pub async fn link_user(ctx: &SerenityContext, user: &User, mc_name: &str) -> Result<LinkStatus> {
    let data = ctx.data::<BotData>();
    let db = &data.db_handle;
    let api = &data.api_handle;

    let uuid = api.uuid(mc_name).await?;

    let discord = api.linked_discord(db, &uuid).await?;

    match discord {
        None => Ok(LinkStatus::NoDiscord),
        Some(linked) => {
            let discriminator = user
                .discriminator
                .map_or("".to_string(), |d| format!("#{}", d.get()));

            if linked != format!("{}{}", user.name, discriminator) {
                return Ok(LinkStatus::DifferentDiscord {
                    other_discord: linked,
                });
            }

            let (duplicate_discord, duplicate_uuid) = db
                .request(InsertLinkedUser {
                    user: LinkedUser::new(user.id, uuid.clone()),
                })
                .await??;

            if let Some(uuid) = duplicate_uuid {
                return Ok(LinkStatus::DuplicateMinecraft {
                    other_username: api.username(&uuid).await?,
                });
            }
            if let Some(discord) = duplicate_discord {
                return Ok(LinkStatus::DuplicateDiscord {
                    uuid,
                    other_discord: discord,
                });
            }

            Ok(LinkStatus::Success)
        }
    }
}

pub enum RoleRequestStatus {
    Updated {
        added: Vec<RoleId>,
        removed: Vec<RoleId>,
        roles: PlayerRoles,
    },
    NoChanges {
        roles: PlayerRoles,
    },
}

impl RoleRequestStatus {
    pub fn get_roles(&self) -> &PlayerRoles {
        match self {
            RoleRequestStatus::Updated { roles, .. } => roles,
            RoleRequestStatus::NoChanges { roles } => roles,
        }
    }

    pub fn to_diff_message(&self, other_user: Option<&UserId>) -> CreateComponent<'static> {
        let user_mention = match other_user {
            Some(id) => format!("{}'s", id.mention()),
            None => "Your".to_string(),
        };

        let unlink_button = CreateButton::new("role:request:unlink")
            .label("Unlink Account")
            .style(ButtonStyle::Danger);

        let diff_text = match self {
            RoleRequestStatus::Updated { added, removed, .. } => {
                let added_mentions: String = if added.is_empty() {
                    "*None*".to_string()
                } else {
                    added
                        .iter()
                        .map(|role| role.mention().to_string())
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                let removed_mentions: String = if removed.is_empty() {
                    "*None*".to_string()
                } else {
                    removed
                        .iter()
                        .map(|role| role.mention().to_string())
                        .collect::<Vec<_>>()
                        .join("\n")
                };

                CreateContainerComponent::TextDisplay(CreateTextDisplay::new(format!(
                    "## Roles Updated Successfully
{user_mention} roles were updated successfully.
### Added
{added_mentions}
### Removed
{removed_mentions}"
                )))
            }
            RoleRequestStatus::NoChanges { .. } => {
                CreateContainerComponent::TextDisplay(CreateTextDisplay::new(format!(
                    "## No Changes
{user_mention} roles weren't modified.",
                )))
            }
        };

        let stats_section = match other_user {
            Some(_) => CreateContainerComponent::TextDisplay(self.get_roles().to_text_display()),
            None => CreateContainerComponent::Section(CreateSection::new(
                vec![CreateSectionComponent::TextDisplay(
                    self.get_roles().to_text_display(),
                )],
                CreateSectionAccessory::Button(unlink_button),
            )),
        };

        let container = CreateContainer::new(vec![diff_text, stats_section]);

        match self {
            RoleRequestStatus::Updated { .. } => {
                CreateComponent::Container(container.accent_color(POSITIVE))
            }
            RoleRequestStatus::NoChanges { .. } => {
                CreateComponent::Container(container.accent_color(YELLOW))
            }
        }
    }
}

pub struct PlayerRoles {
    pub username: String,
    pub blackouts: Vec<Bingo>,
    pub bingo_rank: u8,
    pub immortal: bool,
    pub network_bingos: Vec<NetworkBingo>,
}

impl PlayerRoles {
    pub fn to_text_display(&self) -> CreateTextDisplay<'static> {
        let blackout_list = format!(
            "Total: {}\n{}",
            self.blackouts.len(),
            self.blackouts
                .iter()
                .map(|b| format!("- {b}"))
                .collect::<Vec<_>>()
                .join("\n")
        );

        let network_bingo_list = if self.network_bingos.is_empty() {
            "*None*".to_string()
        } else {
            self.network_bingos
                .iter()
                .map(|n| format!("- {n}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let bingo_rank = if self.bingo_rank == 0 {
            "*None*".to_string()
        } else {
            format!("Rank #{}", self.bingo_rank)
        };

        CreateTextDisplay::new(format!(
            "## Detected Stats for `{}`
### Bingo Rank
{bingo_rank}
### Blackouts
{blackout_list}
### Network Bingos
{network_bingo_list}
\n-# Please report any issues to {}.",
            self.username,
            BOT_MAINTAINER.mention()
        ))
    }
}

pub async fn update_roles(
    ctx: &SerenityContext,
    uuid: &str,
    discord_user: &Member,
) -> Result<RoleRequestStatus> {
    let db = &ctx.data::<BotData>().db_handle;

    let discord_roles = Arc::new(discord_user.roles.clone());

    let player_roles = player_roles(ctx, uuid).await?;

    let mut role_delta = db
        .request(BuildRoleDeltaCompletions {
            bingos: player_roles.blackouts.clone(),
            user_roles: Arc::clone(&discord_roles),
        })
        .await??;

    role_delta.merge(
        db.request(BuildRoleDeltaNetworkBingos {
            bingos: player_roles.network_bingos.clone(),
            user_roles: Arc::clone(&discord_roles),
        })
        .await??,
    );

    role_delta.merge(
        db.request(BuildRoleDeltaBingoRank {
            rank: player_roles.bingo_rank,
            user_roles: Arc::clone(&discord_roles),
        })
        .await??,
    );

    role_delta.merge(
        db.request(BuildRoleDeltaImmortal {
            has_achieved: player_roles.immortal,
            user_roles: Arc::clone(&discord_roles),
        })
        .await??,
    );

    let guild_roles = discord_user
        .guild_id
        .roles(ctx.http())
        .await?
        .into_iter()
        .collect();

    let role_delta = role_delta.resolve(db, guild_roles).await?;

    if role_delta.is_empty() {
        return Ok(RoleRequestStatus::NoChanges {
            roles: player_roles,
        });
    }

    role_delta
        .apply(ctx.http(), discord_user)
        .await
        .context("Failed to update user's roles")?;

    Ok(RoleRequestStatus::Updated {
        added: role_delta.add,
        removed: role_delta.remove,
        roles: player_roles,
    })
}

pub async fn player_roles(ctx: &SerenityContext, uuid: &str) -> Result<PlayerRoles> {
    let data = ctx.data::<BotData>();
    let db = &data.db_handle;
    let api = &data.api_handle;

    let (current_bingo, _, bingo_end) = api.update_current_bingo(db).await?;
    let bingo_ended = Utc::now().timestamp() > bingo_end;
    let current_network_bingo = NetworkBingo::ALL.last().map(|b| *b as u8).unwrap_or(0);
    let network_bingo_active = db.request(GetIsNetworkBingo).await??.unwrap_or(false);

    let bingo_completions = db
        .request(GetBingoData {
            bingo_ids: match db
                .request(CachedCompletions {
                    uuid: uuid.to_string(),
                })
                .await??
            {
                // cache hit
                Some(bitset) => bitset
                    .get_all_set()
                    .into_iter()
                    .map(|id| id as u8)
                    .collect(),
                // cache miss
                None => {
                    let completions = api.bingo_completions(uuid).await?;
                    if bingo_ended || completions.contains(&current_bingo.get_id()) {
                        db.request(CacheCompletions {
                            uuid: uuid.to_string(),
                            completions: BitSet::from_indexes(&completions),
                        })
                        .await??;
                    }
                    completions
                }
            },
        })
        .await??;

    let current_bingo_completed = bingo_completions
        .iter()
        .any(|b| b.get_id() == current_bingo.get_id());

    let network_bingo_completions: Vec<NetworkBingo> = match db
        .request(CachedNetworkBingos {
            uuid: uuid.to_string(),
        })
        .await??
    {
        // cache hit
        Some(bitset) => bitset
            .get_all_set()
            .into_iter()
            .map(|id| NetworkBingo::from_u8(id as u8))
            .collect(),
        // cache miss
        None => {
            let completions = api.network_bingo_completions(db, uuid).await?;
            if completions.contains(&NetworkBingo::from_u8(current_network_bingo))
                || !network_bingo_active
            {
                db.request(CacheNetworkBingos {
                    uuid: uuid.to_string(),
                    completions: BitSet::from_indexes(
                        &completions.iter().map(|b| *b as u8).collect::<Vec<_>>(),
                    ),
                })
                .await??;
            }
            completions
        }
    }
    .into_iter()
    .collect();

    let cached_bingo_rank = db
        .request(CachedBingoRank {
            uuid: uuid.to_string(),
        })
        .await??;
    let cached_immortal = db
        .request(CachedImmortal {
            uuid: uuid.to_string(),
        })
        .await??;

    let (bingo_rank, immortal) = match (cached_bingo_rank, cached_immortal) {
        // cache hit on both bingo rank and immortal
        (Some(rank), Some(immortal)) => (rank, immortal),
        // cache miss on either bingo rank or immortal
        _ => {
            let profile_data = api.bingo_profile_data(uuid).await?;

            if let Some(data) = profile_data {
                let immortal = match cached_immortal {
                    Some(cached) => cached,
                    // Only cache new Immortal status if cache miss, even when fetching bingo profile
                    // data regardless (once accomplished, cache won't invalidate -> no revoking)
                    None => {
                        let current_immortal = !data.has_deaths
                            && bingo_completions
                                .iter()
                                .any(|b| b.get_id() == data.created_during);

                        if bingo_ended || current_bingo_completed {
                            db.request(CacheImmortal {
                                uuid: uuid.to_string(),
                                has_achieved: current_immortal,
                            })
                            .await??;
                        }
                        current_immortal
                    }
                };

                if bingo_ended || data.created_during == current_bingo.get_id() {
                    db.request(CacheBingoRank {
                        uuid: uuid.to_string(),
                        rank: data.bingo_rank,
                    })
                    .await??;
                }

                (data.bingo_rank, immortal)
            } else {
                warn!("No Bingo profile found for '{uuid}'");
                (0, false)
            }
        }
    };

    let username = api.username(uuid).await?;

    Ok(PlayerRoles {
        username,
        blackouts: bingo_completions,
        bingo_rank,
        immortal,
        network_bingos: network_bingo_completions,
    })
}
