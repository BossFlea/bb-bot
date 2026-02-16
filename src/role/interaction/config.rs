use std::{borrow::Cow, sync::Arc};

use anyhow::{Context as _, Result, anyhow, bail};
use either::Either;
use poise::serenity_prelude::{
    CacheHttp as _, ComponentInteraction, ComponentInteractionDataKind, Context as SerenityContext,
    CreateComponent, CreateContainer, CreateContainerComponent, CreateInteractionResponse,
    CreateInteractionResponseFollowup, CreateInteractionResponseMessage, CreateTextDisplay,
    GuildId, Mentionable as _, MessageFlags, ModalInteraction, Role, RoleId,
    colours::css::{DANGER, POSITIVE},
};
use tracing::{info, warn};

use crate::role::{
    db::role_config::{
        DeleteRoleMappingByRole, GetRolePatterns, InsertRoleMapping, SetRolePatterns,
    },
    interaction::modal,
    menu::RoleConfigSession,
    types::{NetworkBingo, RoleMapping, RoleMappingKind, RoleMappingKindRaw, RolePatterns},
};
use crate::shared::{
    BotData,
    interaction::{MessageEdit, modal as shared_modal},
    menu::navigation::GenerateMenu as _,
    types::{Bingo, BingoKind},
};
use crate::{error::UserError, role::db::role_config::DetectRelevantRoles};

pub async fn handle_interaction(
    ctx: &SerenityContext,
    interaction: Either<&ComponentInteraction, &ModalInteraction>,
    mut action: impl Iterator<Item = &str>,
) -> Result<()> {
    match interaction {
        Either::Left(component_interaction) => {
            info!(
                "{} triggered component interaction: '{}'",
                component_interaction.user.name, component_interaction.data.custom_id
            );
        }
        Either::Right(modal_interaction) => {
            info!(
                "{} triggered modal interaction: '{}'",
                modal_interaction.user.name, modal_interaction.data.custom_id
            );
        }
    }

    let menu_id = action
        .next()
        .unwrap_or_default()
        .parse::<u64>()
        .context("Invalid interaction: Expected menu ID")?;

    // NOTE: lock dropped at the end of the expression
    let session_mutex = Arc::clone(
        ctx.data::<BotData>()
            .role_sessions
            .lock()
            .await
            .get(&menu_id)
            .context(UserError(anyhow!("This menu has expired!")))?,
    );

    let mut session = session_mutex.lock().await;
    let (owner_id, owner_name) = &session.owner;

    if let Either::Left(component_interaction) = &interaction
        && component_interaction.user.id != *owner_id
    {
        warn!(
            "{} tried to interact with {}'s menu",
            component_interaction.user.name, owner_name
        );

        let container = CreateComponent::Container(
            CreateContainer::new(vec![CreateContainerComponent::TextDisplay(
                CreateTextDisplay::new(format!(
                    "## You don't own this menu!
Only {} is allowed to interact with this menu.",
                    owner_id.mention()
                )),
            )])
            .accent_color(DANGER),
        );

        let message = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::default()
                .flags(MessageFlags::IS_COMPONENTS_V2)
                .components(vec![container])
                .ephemeral(true),
        );

        component_interaction
            .create_response(ctx.http(), message)
            .await?;
        return Ok(());
    }

    session.timeout_reset.notify_one();

    let new_content = match interaction {
        Either::Left(component_interaction) => {
            component(ctx, component_interaction, action, &mut session).await?
        }
        Either::Right(modal_interaction) => {
            modal(ctx, modal_interaction, action, &mut session).await?
        }
    };

    match new_content {
        MessageEdit::Interaction(menu) => {
            interaction
                .left()
                .context("Invalid edit method for modal interaction")?
                .create_response(
                    ctx.http(),
                    CreateInteractionResponse::UpdateMessage(menu.into_interaction_response()),
                )
                .await?
        }
        MessageEdit::Direct(menu) => {
            ctx.http()
                .edit_message(
                    session.channel_id,
                    session.message_id,
                    &menu.into_edit(),
                    vec![],
                )
                .await?;
        }
        MessageEdit::NoEdit => (),
    }

    Ok(())
}

async fn component(
    ctx: &SerenityContext,
    interaction: &ComponentInteraction,
    mut action: impl Iterator<Item = &str>,
    session: &mut RoleConfigSession,
) -> Result<MessageEdit<'static>> {
    let db = &ctx.data::<BotData>().db_handle;
    let id_prefix = format!("role:config:{}", session.menu_id);

    match action.next().unwrap_or_default() {
        "auto_detect" => {
            interaction.defer_ephemeral(ctx.http()).await?;

            let guild = interaction.guild_id.context(UserError(anyhow!(
                "Interaction triggered outside of a guild"
            )))?;

            let guild_roles: Vec<Role> = guild.roles(ctx.http()).await?.into_iter().collect();

            let detected_roles = db
                .request(DetectRelevantRoles { roles: guild_roles })
                .await??;

            let role_list = if detected_roles.is_empty() {
                Cow::Borrowed("*None*")
            } else {
                Cow::Owned(
                    detected_roles
                        .into_iter()
                        .map(RoleMapping::to_list_entry)
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            };

            let container = CreateComponent::Container(
                CreateContainer::new(vec![CreateContainerComponent::TextDisplay(
                    CreateTextDisplay::new(format!(
                        "## Successfully ran detection
### New detected roles
{role_list}

-# Note:
-# - Network Bingo roles aren't supported for automatic detection due to naming inconsistencies. \
Use the the configuration menu to manually assign them.
-# - For roles with incrementally increasing numbers (e.g. Blackout counts), \
detection stops after not finding a matching role for 3 consecutive numbers. \
If such a gap is intentional, roles can still be configured manually."
                    )),
                )])
                .accent_color(POSITIVE),
            );

            interaction
                .create_followup(
                    ctx.http(),
                    CreateInteractionResponseFollowup::new()
                        .flags(MessageFlags::IS_COMPONENTS_V2)
                        .components(vec![container])
                        .ephemeral(true),
                )
                .await?;

            Ok(MessageEdit::Direct(
                session.state.generate(db, session.menu_id).await?,
            ))
        }
        "edit_patterns" => {
            let patterns = db.request(GetRolePatterns).await??;

            let modal = modal::RolePatterns::create_prefilled(
                &id_prefix,
                patterns.bingo_rank.unwrap_or_default().into(),
                patterns.completions.unwrap_or_default().into(),
                patterns.specific_completion.unwrap_or_default().into(),
                patterns.immortal.unwrap_or_default().into(),
            );

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Modal(modal))
                .await?;
            Ok(MessageEdit::NoEdit)
        }
        "category" => {
            let selected_category = if let ComponentInteractionDataKind::StringSelect { values } =
                &interaction.data.kind
            {
                values
                    .first()
                    .context("Invalid interaction: Expected selected option")?
            } else {
                bail!("Invalid interaction: Expected String SelectMenu")
            };

            let new_category = match selected_category.as_str() {
                "bingo_rank" => RoleMappingKindRaw::BingoRank,
                "completions" => RoleMappingKindRaw::Completions,
                "specific_completion" => RoleMappingKindRaw::SpecificCompletion,
                "network_bingo" => RoleMappingKindRaw::NetworkBingo,
                "immortal" => RoleMappingKindRaw::Immortal,
                _ => bail!("Invalid interaction: Unexpected category identifier"),
            };

            session.state.kind = new_category;

            Ok(MessageEdit::Interaction(
                session.state.generate(db, session.menu_id).await?,
            ))
        }
        "create_mapping" => {
            let modal = match &session.state.kind {
                RoleMappingKindRaw::Completions => {
                    modal::RoleMappingCompletions::create(&id_prefix)
                }
                RoleMappingKindRaw::SpecificCompletion => {
                    modal::RoleMappingSpecificCompletion::create(&id_prefix)
                }
                RoleMappingKindRaw::BingoRank => modal::RoleMappingBingoRank::create(&id_prefix),
                RoleMappingKindRaw::Immortal => modal::RoleMappingImmortal::create(&id_prefix),
                RoleMappingKindRaw::NetworkBingo => {
                    modal::RoleMappingNetworkBingo::create(&id_prefix)
                }
            };

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Modal(modal))
                .await?;

            Ok(MessageEdit::NoEdit)
        }
        "delete_mapping" => {
            let role_id = RoleId::new(
                action
                    .next()
                    .context("Invalid interaction: Expected additional argument")?
                    .parse()
                    .context("Invalid interaction: Expected role ID")?,
            );

            db.request(DeleteRoleMappingByRole { role: role_id })
                .await??;

            Ok(MessageEdit::Interaction(
                session.state.generate(db, session.menu_id).await?,
            ))
        }
        "goto_page" => {
            session.state.page = match action.next().unwrap_or_default() {
                "next" => session.state.page + 1,
                "prev" => session.state.page.saturating_sub(1),
                _ => 0,
            };

            Ok(MessageEdit::Interaction(
                session.state.generate(db, session.menu_id).await?,
            ))
        }
        "jump_page" => {
            let modal = shared_modal::JumpPage::create(&id_prefix);

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Modal(modal))
                .await?;
            Ok(MessageEdit::NoEdit)
        }
        _ => Err(anyhow!("Invalid interaction: Unexpected action")),
    }
}

async fn modal(
    ctx: &SerenityContext,
    interaction: &ModalInteraction,
    mut action: impl Iterator<Item = &str>,
    session: &mut RoleConfigSession,
) -> Result<MessageEdit<'static>> {
    let db = &ctx.data::<BotData>().db_handle;

    match action.next().unwrap_or_default() {
        "jump_page_submit" => {
            let values = shared_modal::JumpPage::validate(&interaction.data.components)?;

            if values.page.is_empty() {
                return Ok(MessageEdit::NoEdit);
            };
            let jump_page: usize = values.page.parse().unwrap_or(0);
            session.state.page = jump_page.saturating_sub(1);

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Acknowledge)
                .await?;
            let menu = session.state.generate(db, session.menu_id).await?;
            Ok(MessageEdit::Direct(menu))
        }
        "role_patterns_submit" => {
            let values = modal::RolePatterns::validate(&interaction.data.components)?;

            db.request(SetRolePatterns {
                patterns: RolePatterns::new(
                    values.completions.to_string(),
                    values.specific_completion.to_string(),
                    values.bingo_rank.to_string(),
                    values.immortal.to_string(),
                ),
            })
            .await??;

            let container = CreateContainer::new(vec![CreateContainerComponent::TextDisplay(
                CreateTextDisplay::new(
                    "## Successfully Updated Patterns
Set auto-detection patterns successfully. \
Use the `Detect Roles` button in the configuration menu \
to run the detection process using the new patterns.",
                ),
            )])
            .accent_colour(POSITIVE);

            interaction
                .create_response(
                    ctx.http(),
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .flags(MessageFlags::IS_COMPONENTS_V2)
                            .components(vec![CreateComponent::Container(container)])
                            .ephemeral(true),
                    ),
                )
                .await?;

            Ok(MessageEdit::NoEdit)
        }
        "role_mapping_bingo_rank_submit" => {
            let values = modal::RoleMappingBingoRank::validate(&interaction.data.components)?;

            let role_id = validate_role_string(
                ctx,
                interaction
                    .guild_id
                    .as_ref()
                    .expect("Guild ID validated upon receiving interaction"),
                &values.role_id,
            )
            .await?;

            let rank: u8 = values
                .rank
                .parse()
                .context(UserError(anyhow!("Failed to parse rank: Invalid number")))?;

            db.request(InsertRoleMapping {
                role_mapping: RoleMapping::new(RoleMappingKind::BingoRank { rank }, role_id),
            })
            .await??;

            let container = CreateContainer::new(vec![CreateContainerComponent::TextDisplay(
                CreateTextDisplay::new(format!(
                    "## Successfully Added Role Binding
Associated {} with Bingo Rank {rank}.",
                    role_id.mention()
                )),
            )])
            .accent_color(POSITIVE);

            interaction
                .create_response(
                    ctx.http(),
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .flags(MessageFlags::IS_COMPONENTS_V2)
                            .components(vec![CreateComponent::Container(container)])
                            .ephemeral(true),
                    ),
                )
                .await?;

            Ok(MessageEdit::Direct(
                session.state.generate(db, session.menu_id).await?,
            ))
        }
        "role_mapping_completions_submit" => {
            let values = modal::RoleMappingCompletions::validate(&interaction.data.components)?;

            let role_id = validate_role_string(
                ctx,
                interaction
                    .guild_id
                    .as_ref()
                    .expect("Guild ID validated upon receiving interaction"),
                &values.role_id,
            )
            .await?;

            let count: usize = values.count.parse().context(UserError(anyhow!(
                "Failed to parse Blackout count: Invalid number"
            )))?;

            db.request(InsertRoleMapping {
                role_mapping: RoleMapping::new(RoleMappingKind::Completions { count }, role_id),
            })
            .await??;

            let container = CreateContainer::new(vec![CreateContainerComponent::TextDisplay(
                CreateTextDisplay::new(format!(
                    "## Successfully Added Role Binding
Associated {} with Blackout count {count}.",
                    role_id.mention()
                )),
            )])
            .accent_color(POSITIVE);

            interaction
                .create_response(
                    ctx.http(),
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .flags(MessageFlags::IS_COMPONENTS_V2)
                            .components(vec![CreateComponent::Container(container)])
                            .ephemeral(true),
                    ),
                )
                .await?;

            Ok(MessageEdit::Direct(
                session.state.generate(db, session.menu_id).await?,
            ))
        }
        "role_mapping_specific_completion_submit" => {
            let values =
                modal::RoleMappingSpecificCompletion::validate(&interaction.data.components)?;

            let role_id = validate_role_string(
                ctx,
                interaction
                    .guild_id
                    .as_ref()
                    .expect("Guild ID validated upon receiving interaction"),
                &values.role_id,
            )
            .await?;

            let kind_specific_id: u8 = values.kind_specific_id.parse().context(UserError(
                anyhow!("Failed to parse Bingo ID: Invalid number"),
            ))?;

            let bingo_kind = BingoKind::from_u8(
                values
                    .bingo_kind
                    .first()
                    .context("Expected selected option")?
                    .parse()
                    .context("Expected valid Bingo kind ID")?,
            );

            let bingo = Bingo::new(kind_specific_id, bingo_kind, None);

            db.request(InsertRoleMapping {
                role_mapping: RoleMapping::new(
                    RoleMappingKind::SpecificCompletion { bingo },
                    role_id,
                ),
            })
            .await??;

            let container = CreateContainer::new(vec![CreateContainerComponent::TextDisplay(
                CreateTextDisplay::new(format!(
                    "## Successfully Added Role Binding
Associated {} with {bingo}.",
                    role_id.mention()
                )),
            )])
            .accent_color(POSITIVE);

            interaction
                .create_response(
                    ctx.http(),
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .flags(MessageFlags::IS_COMPONENTS_V2)
                            .components(vec![CreateComponent::Container(container)])
                            .ephemeral(true),
                    ),
                )
                .await?;

            Ok(MessageEdit::Direct(
                session.state.generate(db, session.menu_id).await?,
            ))
        }
        "role_mapping_immortal_submit" => {
            let values = modal::RoleMappingImmortal::validate(&interaction.data.components)?;

            let role_id = validate_role_string(
                ctx,
                interaction
                    .guild_id
                    .as_ref()
                    .expect("Guild ID validated upon receiving interaction"),
                &values.role_id,
            )
            .await?;

            db.request(InsertRoleMapping {
                role_mapping: RoleMapping::new(RoleMappingKind::Immortal, role_id),
            })
            .await??;

            let container = CreateContainer::new(vec![CreateContainerComponent::TextDisplay(
                CreateTextDisplay::new(format!(
                    "## Successfully Added Role Binding
Associated {} with Immortal.",
                    role_id.mention()
                )),
            )])
            .accent_color(POSITIVE);

            interaction
                .create_response(
                    ctx.http(),
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .flags(MessageFlags::IS_COMPONENTS_V2)
                            .components(vec![CreateComponent::Container(container)])
                            .ephemeral(true),
                    ),
                )
                .await?;

            Ok(MessageEdit::Direct(
                session.state.generate(db, session.menu_id).await?,
            ))
        }
        "role_mapping_network_bingo_submit" => {
            let values = modal::RoleMappingNetworkBingo::validate(&interaction.data.components)?;

            let role_id = validate_role_string(
                ctx,
                interaction
                    .guild_id
                    .as_ref()
                    .expect("Guild ID validated upon receiving interaction"),
                &values.role_id,
            )
            .await?;

            let bingo = NetworkBingo::from_u8(
                values
                    .bingo
                    .first()
                    .context("Expected selected option")?
                    .parse()
                    .context("Expected valid Network Bingo ID")?,
            );

            db.request(InsertRoleMapping {
                role_mapping: RoleMapping::new(RoleMappingKind::NetworkBingo { bingo }, role_id),
            })
            .await??;

            let container = CreateContainer::new(vec![CreateContainerComponent::TextDisplay(
                CreateTextDisplay::new(format!(
                    "## Successfully Added Role Binding
Associated {} with Network Bingo '{bingo}'.",
                    role_id.mention()
                )),
            )])
            .accent_color(POSITIVE);

            interaction
                .create_response(
                    ctx.http(),
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .flags(MessageFlags::IS_COMPONENTS_V2)
                            .components(vec![CreateComponent::Container(container)])
                            .ephemeral(true),
                    ),
                )
                .await?;

            Ok(MessageEdit::Direct(
                session.state.generate(db, session.menu_id).await?,
            ))
        }
        _ => Err(anyhow!("Invalid interaction: Unexpected action")),
    }
}

async fn validate_role_string(
    ctx: &SerenityContext,
    guild_id: &GuildId,
    role_id: &str,
) -> Result<RoleId> {
    let role_id = RoleId::new(
        role_id
            .parse::<u64>()
            .context("Failed to parse role ID: Invalid format")?,
    );

    ctx.http()
        .get_guild_role(*guild_id, role_id)
        .await
        .context("Failed to validate role ID: Invalid role in current guild")
        .map(|r| r.id)
}
