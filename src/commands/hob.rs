use std::{sync::Arc, time::Duration};

use anyhow::Result;
use poise::{
    CreateReply,
    serenity_prelude::{
        CreateAttachment, CreateComponent, CreateContainer, CreateMessage, CreateTextDisplay,
        GenericChannelId, Mentionable as _, MessageFlags, colours::css::POSITIVE,
    },
};
use tokio::sync::{Mutex, Notify};

use crate::config::{HOB_LOG_CHANNEL, MENU_TIMEOUT_SECS};
use crate::hob::{
    db::GetAllHobEntries,
    menu::{HobEditSession, HobEditState, SelectEntryState, format},
    types::HobEntry,
};
use crate::shared::{
    Context,
    menu::{navigation::GenerateMenu as _, timeout},
};

#[poise::command(slash_command, subcommand_required, subcommands("manage", "send"))]
pub async fn hob(_ctx: Context<'_>) -> Result<()> {
    unreachable!("This shouldn't be possible to invoke");
}

/// Manage the HoB database
#[poise::command(slash_command, required_bot_permissions = "VIEW_CHANNEL")]
async fn manage(ctx: Context<'_>) -> Result<()> {
    let menu_id = crate::shared::menu::generate_id();
    let mut initial_state = SelectEntryState {
        page: 0,
        search_query: None,
    };
    let menu = initial_state
        .generate(&ctx.data().db_handle, menu_id)
        .await?;

    let message_handle = ctx.send(menu.into_reply()).await?.into_message().await?;

    let session = HobEditSession {
        menu_id,
        state: HobEditState::SelectEntry(initial_state),
        owner: ctx.author().clone(),
        channel_id: message_handle.channel_id,
        message_id: message_handle.id,
        timeout_reset: Arc::new(Notify::new()),
    };

    timeout::spawn_timeout(
        Arc::clone(&ctx.serenity_context().http),
        Arc::clone(&ctx.data().hob_sessions),
        session.menu_id,
        Duration::from_secs(MENU_TIMEOUT_SECS),
        Arc::clone(&session.timeout_reset),
    )
    .await;

    {
        let data_arc = ctx.data();
        let mut hob_sessions = data_arc.hob_sessions.lock().await;
        hob_sessions.insert(session.menu_id, Arc::new(Mutex::new(session)));
    }
    Ok(())
}

/// Send the HoB list
#[poise::command(
    slash_command,
    required_bot_permissions = "VIEW_CHANNEL | SEND_MESSAGES | ATTACH_FILES"
)]
async fn send(
    ctx: Context<'_>,
    #[description = "Where to send the HoB list"]
    #[channel_types("Text")]
    channel: Option<GenericChannelId>,
    #[description = "Whether to suppress backup script generation"] suppress_backup_script: Option<
        bool,
    >,
) -> Result<()> {
    let channel = channel.unwrap_or(ctx.channel_id());
    let suppress_backup_script = suppress_backup_script.unwrap_or(false);

    let hob_entries = ctx.data().db_handle.request(GetAllHobEntries).await??;

    let containers = format::build_hob_messages(&hob_entries, 5)?;
    let message_count = containers.len();

    let messages = containers.into_iter().map(|container| {
        CreateMessage::new()
            .flags(MessageFlags::IS_COMPONENTS_V2)
            .components(vec![container])
    });

    ctx.defer_ephemeral().await?;

    for message in messages {
        channel.send_message(ctx.http(), message).await?;
    }

    if !suppress_backup_script {
        HOB_LOG_CHANNEL
            .send_message(ctx.http(), log_message(&hob_entries)?)
            .await?;
    }

    let container = CreateComponent::Container(
        CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
            format!(
                "## Sent Successfully
The full HoB was sent to {} in the form of {message_count} messages.",
                channel.mention()
            ),
        ))])
        .accent_color(POSITIVE),
    );

    ctx.send(
        CreateReply::default()
            .flags(MessageFlags::IS_COMPONENTS_V2)
            .components(vec![container])
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

fn log_message(hob_entries: &[HobEntry]) -> Result<CreateMessage<'static>> {
    let log_text = "## HoB Backup Script
This script resets the tables responsible for storing HoB data to their current state \
when executed on the bot database in the future.";

    let backup_file = CreateAttachment::bytes(
        format::build_hob_backup_script(hob_entries).into_bytes(),
        format!(
            "reset_hob_entries_{}.sql",
            chrono::Utc::now().format("%b%Y")
        ),
    );

    Ok(CreateMessage::new().content(log_text).add_file(backup_file))
}
