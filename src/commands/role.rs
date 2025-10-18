use std::{sync::Arc, time::Duration};

use anyhow::{Context as _, Result, anyhow, bail};
use poise::{
    CreateReply,
    serenity_prelude::{
        ButtonStyle, CreateActionRow, CreateButton, CreateComponent, CreateContainer,
        CreateMessage, CreateSection, CreateSectionAccessory, CreateSectionComponent,
        CreateTextDisplay, EditMessage, GenericChannelId, Member, Mentionable as _, Message,
        MessageFlags, ReactionType, UserId,
        colours::{
            css::{POSITIVE, WARNING},
            roles::BLUE,
        },
        small_fixed_array::FixedString,
    },
};
use tokio::sync::{Mutex, Notify};

use crate::config::{MANUAL_ROLE_CHANNEL, MENU_TIMEOUT_SECS};
use crate::error::UserError;
use crate::role::{
    menu::{RoleConfigSession, RoleConfigState},
    request,
    types::{NetworkBingo, RoleMappingKind, RoleMappingKindRaw},
};
use crate::shared::{
    Context,
    menu::{navigation::GenerateMenu as _, timeout},
    types::{Bingo, BingoKind},
};

#[poise::command(
    slash_command,
    subcommand_required,
    subcommands("send", "force", "config", "query", "network_bingo")
)]
pub async fn rolerequest(_ctx: Context<'_>) -> Result<()> {
    unreachable!("This shouldn't be possible to invoke");
}

/// Send a message with the clickable role request button
#[poise::command(
    slash_command,
    required_bot_permissions = "VIEW_CHANNEL | SEND_MESSAGES | READ_MESSAGE_HISTORY"
)]
async fn send(
    ctx: Context<'_>,
    #[description = "Where to send the message"]
    #[channel_types("Text")]
    channel: Option<GenericChannelId>,
    #[description = "ID of existing message to edit (must belong to bot)"] edit: Option<Message>,
) -> Result<()> {
    let db = &ctx.data().db_handle;

    let begin_button = CreateButton::new("role:request:begin")
        .label("Request Roles")
        .style(ButtonStyle::Primary);
    let faq_button = CreateButton::new("role:request:faq")
        .emoji(ReactionType::Unicode(FixedString::from_static_trunc("ℹ️")))
        // .emoji('❓')
        .label("Common Questions")
        .style(ButtonStyle::Secondary);

    // TODO: add additional unlink button here?
    let text = CreateComponent::TextDisplay(CreateTextDisplay::new(format!(
        "Click the button to update your bingo-related roles.
-# Note: If you've never done this before, you will be prompted to link your Hypixel profile.
### Bingo Rank role
e.g. {} – Mirrors your in-game bingo rank
### Bingo Blackout counter
e.g. {} – Counter of how many bingo cards you've completed
### Special Blackout roles
e.g. {} – Blackout roles for extreme/secret bingo events
### Network Bingo completion roles
e.g. {} – Roles for completing hypixel's seasonal network bingo events
### 'Immortal' role
{} – Awarded for completing a bingo card without dying a single time
-# Note: This will only work if your bingo profile still has no deaths and hasn't been deleted yet!

**» All other roles are granted manually in {} !**",
        db.get_role(RoleMappingKind::BingoRank { rank: 4 })
            .await?
            .unwrap_or_default()
            .mention(),
        db.get_role(RoleMappingKind::Completions { count: 12 })
            .await?
            .unwrap_or_default()
            .mention(),
        db.get_role(RoleMappingKind::SpecificCompletion {
            bingo: Bingo::new(2, BingoKind::Extreme, None)
        })
        .await?
        .unwrap_or_default()
        .mention(),
        db.get_role(RoleMappingKind::NetworkBingo {
            bingo: NetworkBingo::Anniversary2023
        })
        .await?
        .unwrap_or_default()
        .mention(),
        db.get_role(RoleMappingKind::Immortal)
            .await?
            .unwrap_or_default()
            .mention(),
        MANUAL_ROLE_CHANNEL.mention()
    )));

    let title_section = CreateComponent::Section(CreateSection::new(
        vec![CreateSectionComponent::TextDisplay(CreateTextDisplay::new(
            "# Automated Role Requests",
        ))],
        CreateSectionAccessory::Button(begin_button),
    ));

    let faq_row = CreateComponent::ActionRow(CreateActionRow::Buttons(vec![faq_button].into()));

    let container = CreateComponent::Container(
        CreateContainer::new(vec![title_section, text, faq_row]).accent_color(BLUE),
    );

    ctx.defer_ephemeral().await?;

    let channel = channel.unwrap_or(ctx.channel_id());

    match edit {
        Some(mut message) => {
            if message.author.id != ctx.framework().bot_id() {
                bail!(UserError(anyhow!("Provided message doesn't belong to bot")))
            }

            message
                .edit(
                    ctx,
                    EditMessage::new()
                        .flags(MessageFlags::IS_COMPONENTS_V2)
                        .components(vec![container]),
                )
                .await?
        }
        None => {
            channel
                .send_message(
                    ctx.http(),
                    CreateMessage::new()
                        .flags(MessageFlags::IS_COMPONENTS_V2)
                        .components(vec![container]),
                )
                .await?;
        }
    }

    let reply_container = CreateComponent::Container(
        CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
            format!(
                "## Sent Successfully
The role requests message was successfully sent to {}.",
                channel.mention()
            ),
        ))])
        .accent_color(POSITIVE),
    );

    ctx.send(
        CreateReply::default()
            .flags(MessageFlags::IS_COMPONENTS_V2)
            .components(vec![reply_container])
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

/// Configure role-related settings
#[poise::command(
    slash_command,
    required_bot_permissions = "VIEW_CHANNEL | READ_MESSAGE_HISTORY"
)]
async fn config(ctx: Context<'_>) -> Result<()> {
    let menu_id = crate::shared::menu::generate_id();
    let mut initial_state = RoleConfigState::new(RoleMappingKindRaw::BingoRank, 0);

    let menu = initial_state
        .generate(&ctx.data().db_handle, menu_id)
        .await?;

    let message_handle = ctx.send(menu.into_reply()).await?.into_message().await?;

    let session = RoleConfigSession {
        menu_id,
        owner: ctx.author().clone(),
        state: initial_state,
        channel_id: message_handle.channel_id,
        message_id: message_handle.id,
        timeout_reset: Arc::new(Notify::new()),
    };

    timeout::spawn_timeout(
        Arc::clone(&ctx.serenity_context().http),
        Arc::clone(&ctx.data().role_sessions),
        session.menu_id,
        Duration::from_secs(MENU_TIMEOUT_SECS),
        Arc::clone(&session.timeout_reset),
    )
    .await;

    {
        let data_arc = ctx.data();
        let mut role_sessions = data_arc.role_sessions.lock().await;
        role_sessions.insert(session.menu_id, Arc::new(Mutex::new(session)));
    }
    Ok(())
}

/// Set whether the bot considers the latest Network Bingo as currently ongoing. This affects caching.
#[poise::command(slash_command, rename = "networkbingo")]
async fn network_bingo(
    ctx: Context<'_>,
    #[description = "Whether a Network Bingo Event is currently ongoing"] active: bool,
) -> Result<()> {
    ctx.data().db_handle.update_is_network_bingo(active).await?;

    let message = match active {
        true => "Role Requests will no longer serve cached Network Bingo Completions.",
        false => "Cached Network Bingo Completions gathered during the latest event will now be served if present.
-# This is to reduce redundant API requests when there isn't an ongoing Network Bingo event.
-# **Caution**: This has the potential to serve stale data from cache if wrongfully enabled during an active event.",
    };

    let response = CreateReply::new()
        .flags(MessageFlags::IS_COMPONENTS_V2)
        .components(vec![CreateComponent::Container(
            CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
                format!("## Successfully Updated Status\n{message}"),
            ))])
            .accent_colour(POSITIVE),
        )])
        .ephemeral(true);

    ctx.send(response).await?;
    Ok(())
}

#[poise::command(
    slash_command,
    subcommand_required,
    subcommands("force_update", "force_link", "force_unlink")
)]
async fn force(_ctx: Context<'_>) -> Result<()> {
    unreachable!("This shouldn't be possible to invoke");
}

/// Update another user's roles (use '/rolerequest inspect stats' to check stats without updating roles)
#[poise::command(slash_command, rename = "update")]
async fn force_update(
    ctx: Context<'_>,
    #[description = "Whose roles to update"] user: Member,
) -> Result<()> {
    let uuid = ctx
        .data()
        .db_handle
        .get_linked_user_by_discord(user.user.id)
        .await?
        .context(UserError(anyhow!("User hasn't linked their accounts")))?;

    ctx.defer().await?;

    let role_status = request::update_roles(ctx.serenity_context(), &uuid, &user).await?;

    let container = role_status.to_diff_message(Some(&user.user.id));

    ctx.send(
        CreateReply::default()
            .flags(MessageFlags::IS_COMPONENTS_V2)
            .components(vec![container]), // .ephemeral(true),
    )
    .await?;

    Ok(())
}

/// Override another user's linked minecraft account
#[poise::command(slash_command, rename = "link")]
async fn force_link(
    ctx: Context<'_>,
    #[description = "Whose linked account to update"] discord: UserId,
    #[description = "Minecraft account to link"] minecraft: String,
) -> Result<()> {
    ctx.defer().await?;

    let uuid = ctx.data().api_handle.uuid(&minecraft).await?;

    ctx.data()
        .db_handle
        .update_linked_user(discord, uuid)
        .await?;

    let container = CreateComponent::Container(
        CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
            format!(
                "## Linked Successfully
Linked `{minecraft}` to {}, discarding any existing links for either account.",
                discord.mention()
            ),
        ))])
        .accent_color(POSITIVE),
    );

    ctx.send(
        CreateReply::default()
            .flags(MessageFlags::IS_COMPONENTS_V2)
            .components(vec![container]), // .ephemeral(true),
    )
    .await?;

    Ok(())
}

/// Unlink another user's minecraft account
#[poise::command(slash_command, rename = "unlink")]
async fn force_unlink(
    ctx: Context<'_>,
    #[description = "Whose account to unlink"] user: UserId,
) -> Result<()> {
    ctx.defer().await?;

    let removed_uuid = ctx
        .data()
        .db_handle
        .remove_linked_user_by_discord(user)
        .await?
        .context(UserError(anyhow!("User hasn't linked their accounts",)))?;

    let username = ctx.data().api_handle.username(&removed_uuid).await?;

    let container = CreateComponent::Container(
        CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
            format!(
                "## Unlinked Successfully
Unlinked `{username}` from {}.",
                user.mention()
            ),
        ))])
        .accent_color(POSITIVE),
    );

    ctx.send(
        CreateReply::default()
            .flags(MessageFlags::IS_COMPONENTS_V2)
            .components(vec![container]), // .ephemeral(true),
    )
    .await?;

    Ok(())
}

/// Check the link status and stats of any Discord/Minecraft account
#[poise::command(slash_command)]
async fn query(
    ctx: Context<'_>,
    #[description = "By Discord account"] discord: Option<UserId>,
    #[description = "By Minecraft username/UUID"] minecraft: Option<String>,
) -> Result<()> {
    let db = &ctx.data().db_handle;
    let api = &ctx.data().api_handle;

    ctx.defer().await?;

    let (discord, uuid, username) = match (discord, minecraft) {
        (None, None) => {
            let text = CreateComponent::TextDisplay(CreateTextDisplay::new(
                "## Insufficient arguments
You need to provide either the `discord` or `minecraft` command argument.",
            ));
            let container =
                CreateComponent::Container(CreateContainer::new(vec![text]).accent_color(WARNING));
            ctx.send(
                CreateReply::new()
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(vec![container]),
            )
            .await?;
            return Ok(());
        }
        (Some(user_id), _) => {
            let uuid = match db.get_linked_user_by_discord(user_id).await? {
                Some(uuid) => uuid,
                None => {
                    let text = format!(
                        "## Unlinked
{} hasn't linked a Minecraft account to their Discord.",
                        user_id.mention()
                    );
                    let container = CreateComponent::Container(
                        CreateContainer::new(vec![CreateComponent::TextDisplay(
                            CreateTextDisplay::new(text),
                        )])
                        .accent_color(WARNING),
                    );
                    ctx.send(
                        CreateReply::new()
                            .flags(MessageFlags::IS_COMPONENTS_V2)
                            .components(vec![container]),
                    )
                    .await?;
                    return Ok(());
                }
            };
            let username = api.username(&uuid).await?;
            (Some(user_id), uuid, username)
        }
        (_, Some(minecraft)) => {
            let (uuid, username) = if minecraft.len() >= 32 {
                let username = api
                    .username(&minecraft)
                    .await
                    .map_err(|err| anyhow!(UserError(err)))?;
                (minecraft, username)
            } else {
                let uuid = api.uuid(&minecraft).await?;
                (uuid, minecraft)
            };
            let discord = db.get_linked_user_by_uuid(uuid.clone()).await?;
            (discord, uuid, username)
        }
    };

    let link_message = match discord {
        Some(user_id) => {
            format!(
                "## Linked
{} is linked to `{username}`.
-# Stored in database as UUID.",
                user_id.mention()
            )
        }
        None => {
            format!(
                "## Unlinked
`{username}` hasn't been linked to a Discord account."
            )
        }
    };
    let link_text = CreateComponent::TextDisplay(CreateTextDisplay::new(link_message));

    let roles_text = CreateComponent::TextDisplay(
        request::player_roles(ctx.serenity_context(), &uuid)
            .await?
            .to_text_display(),
    );

    let container = CreateComponent::Container(
        CreateContainer::new(vec![link_text, roles_text]).accent_color(POSITIVE),
    );

    ctx.send(
        CreateReply::new()
            .flags(MessageFlags::IS_COMPONENTS_V2)
            .components(vec![container]),
    )
    .await?;
    Ok(())
}
