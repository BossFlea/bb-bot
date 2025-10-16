use std::sync::Arc;

use anyhow::{Context as _, Result};
use poise::serenity_prelude::colours::branding::YELLOW;
use poise::serenity_prelude::colours::css::POSITIVE;
use poise::serenity_prelude::{
    ButtonStyle, CacheHttp as _, Context as SerenityContext, CreateButton, CreateComponent,
    CreateContainer, CreateSection, CreateSectionAccessory, CreateSectionComponent,
    CreateTextDisplay, Member, Mentionable as _, RoleId, User, UserId,
};
use tracing::warn;

use crate::config::BOT_MAINTAINER;
use crate::role::types::{LinkStatus, NetworkBingo};
use crate::shared::{
    BotData,
    types::{Bingo, BitSet},
};

pub async fn link_user(ctx: &SerenityContext, user: &User, mc_name: &str) -> Result<LinkStatus> {
    let data = ctx.data::<BotData>();
    let db = &data.db_handle;
    let api = &data.api_handle;

    let uuid = api.uuid(mc_name).await?;

    let discord = api.linked_discord(&uuid).await?;

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

            let (duplicate_discord, duplicate_uuid) =
                db.insert_linked_user(user.id, uuid.clone()).await?;

            if let Some(uuid) = duplicate_uuid {
                return Ok(LinkStatus::AlreadyLinked {
                    other_username: api.username(&uuid).await?,
                });
            }
            if let Some(discord) = duplicate_discord {
                return Ok(LinkStatus::DuplicateLink {
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

                CreateComponent::TextDisplay(CreateTextDisplay::new(format!(
                    "## Roles Updated Successfully
{user_mention} roles were updated successfully.
### Added
{added_mentions}
### Removed
{removed_mentions}"
                )))
            }
            RoleRequestStatus::NoChanges { .. } => {
                CreateComponent::TextDisplay(CreateTextDisplay::new(format!(
                    "## No Changes
{user_mention} roles weren't modified.",
                )))
            }
        };

        let stats_section = match other_user {
            Some(_) => CreateComponent::TextDisplay(self.get_roles().to_text_display()),
            None => CreateComponent::Section(CreateSection::new(
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

    let user_roles = player_roles(ctx, uuid).await?;

    let mut role_delta = db
        .get_roles_from_bingos(Arc::clone(&discord_roles), user_roles.blackouts.clone())
        .await?;

    role_delta.merge(
        db.get_roles_from_network_bingos(
            Arc::clone(&discord_roles),
            user_roles.network_bingos.clone(),
        )
        .await?,
    );

    role_delta.merge(
        db.get_roles_bingo_rank(Arc::clone(&discord_roles), user_roles.bingo_rank)
            .await?,
    );

    if user_roles.immortal
        && let Some(role) = db.get_role_immortal(Arc::clone(&discord_roles)).await?
    {
        role_delta.add.push(role);
    }

    let guild_roles = discord_user
        .guild_id
        .roles(ctx.http())
        .await?
        .into_iter()
        .collect();

    let role_delta = role_delta.resolve(db, guild_roles).await?;

    if role_delta.is_empty() {
        return Ok(RoleRequestStatus::NoChanges { roles: user_roles });
    }

    role_delta
        .apply(ctx.http(), discord_user)
        .await
        .context("Failed to update user's roles")?;

    Ok(RoleRequestStatus::Updated {
        added: role_delta.add,
        removed: role_delta.remove,
        roles: user_roles,
    })
}

pub async fn player_roles(ctx: &SerenityContext, uuid: &str) -> Result<PlayerRoles> {
    let data = ctx.data::<BotData>();
    let db = &data.db_handle;
    let api = &data.api_handle;

    let (current_bingo, bingo_active) = api.update_current_bingo(db).await?;
    let current_network_bingo = NetworkBingo::ALL.last().map(|b| *b as u8).unwrap_or(0);
    let network_bingo_active = db.get_is_network_bingo().await?.unwrap_or(false);

    let bingo_completions = db
        .complete_bingo_data(
            match db.cache_lookup_completions(uuid.to_string(), 0).await? {
                // cache hit
                Some(bitset) => bitset
                    .get_all_set()
                    .into_iter()
                    .map(|id| id as u8)
                    .collect(),
                // cache miss
                None => {
                    let completions = api.bingo_completions(uuid).await?;
                    if !bingo_active || completions.contains(&current_bingo.get_id()) {
                        db.cache_insert_completions(
                            uuid.to_string(),
                            current_bingo.get_id(),
                            BitSet::from_indexes(&completions),
                        )
                        .await?;
                    }
                    completions
                }
            },
        )
        .await?;

    let current_bingo_completed = bingo_completions
        .iter()
        .any(|b| b.get_id() == current_bingo.get_id());

    let network_bingo_completions: Vec<NetworkBingo> = match db
        .cache_lookup_network_completions(uuid.to_string(), current_network_bingo)
        .await?
    {
        // cache hit
        Some(bitset) => bitset
            .get_all_set()
            .into_iter()
            .map(|id| NetworkBingo::from_u8(id as u8))
            .collect(),
        // cache miss
        None => {
            let completions = api.network_bingo_completions(uuid).await?;
            if completions.contains(&NetworkBingo::from_u8(current_network_bingo))
                || !network_bingo_active
            {
                db.cache_insert_network_completions(
                    uuid.to_string(),
                    current_network_bingo,
                    BitSet::from_indexes(&completions.iter().map(|b| *b as u8).collect::<Vec<_>>()),
                )
                .await?;
            }
            completions
        }
    }
    .into_iter()
    .collect();

    let cached_bingo_rank = db
        .cache_lookup_bingo_rank(uuid.to_string(), current_bingo.get_id())
        .await?;
    let cached_immortal = db
        .cache_lookup_immortal(uuid.to_string(), current_bingo.get_id())
        .await?;

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

                        if !bingo_active || current_bingo_completed {
                            db.cache_insert_immortal(
                                uuid.to_string(),
                                current_bingo.get_id(),
                                current_immortal,
                            )
                            .await?;
                        }
                        current_immortal
                    }
                };

                if data.created_during == current_bingo.get_id() {
                    db.cache_insert_bingo_rank(
                        uuid.to_string(),
                        current_bingo.get_id(),
                        data.bingo_rank,
                    )
                    .await?;
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
