use std::{sync::Arc, time::Duration};

use anyhow::Result;
use poise::{
    CreateReply,
    serenity_prelude::{
        Component, CreateAttachment, CreateComponent, CreateContainer, CreateContainerComponent,
        CreateMessage, CreateTextDisplay, GenericChannelId, GetMessages, Http, Mentionable as _,
        Message, MessageFlags, MessageId, UserId, colours::css::POSITIVE,
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

    let owner = (ctx.author().id, ctx.author().name.to_string());

    let session = HobEditSession {
        menu_id,
        state: HobEditState::SelectEntry(initial_state),
        owner,
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
    required_bot_permissions = "VIEW_CHANNEL | SEND_MESSAGES | ATTACH_FILES | READ_MESSAGE_HISTORY"
)]
async fn send(
    ctx: Context<'_>,
    #[description = "Where to send the HoB list"]
    #[channel_types("Text")]
    channel: Option<GenericChannelId>,
    #[description = "Whether to suppress backup script generation"] suppress_backup_script: Option<
        bool,
    >,
    #[description = "Whether to skip deletion of previously sent HoB messages in the target channel"]
    skip_deletion: Option<bool>,
) -> Result<()> {
    let channel = channel.unwrap_or(ctx.channel_id());
    let suppress_backup_script = suppress_backup_script.unwrap_or(false);
    let skip_deletion = skip_deletion.unwrap_or(false);

    let hob_entries = ctx.data().db_handle.request(GetAllHobEntries).await??;

    let containers = format::build_hob_messages(&hob_entries, 5)?;
    let message_count = containers.len();

    let messages = containers.into_iter().map(|container| {
        CreateMessage::new()
            .flags(MessageFlags::IS_COMPONENTS_V2)
            .components(vec![container])
    });

    ctx.defer_ephemeral().await?;

    let mut sent_message_ids = Vec::with_capacity(message_count);
    for message in messages {
        sent_message_ids.push(channel.send_message(ctx.http(), message).await?.id);
    }

    let deletion_feedback = if skip_deletion {
        "Skipped deletion of previous HoB messages.".to_string()
    } else if let Some(oldest_sent_message_id) = sent_message_ids.first() {
        match delete_previous_hob_messages(
            ctx.http(),
            channel,
            ctx.framework().bot_id(),
            *oldest_sent_message_id,
        )
        .await
        {
            Ok(deleted_count) => {
                tracing::info!(
                    "Deleted {deleted_count} previous HoB message(s) in channel {}",
                    channel.get()
                );
                format!("Deleted {deleted_count} previous HoB message(s).")
            }
            Err(error) => {
                tracing::warn!("Failed to delete previous HoB messages: {error:#}");
                "Failed to delete previous HoB messages!".to_string()
            }
        }
    } else {
        "No previous HoB messages were deleted.".to_string()
    };

    let backup_feedback = if suppress_backup_script {
        None
    } else {
        match log_message(&hob_entries) {
            Ok(message) => match HOB_LOG_CHANNEL.send_message(ctx.http(), message).await {
                Ok(_) => Some(format!(
                    "Backup script sent to {}.",
                    HOB_LOG_CHANNEL.mention()
                )),
                Err(error) => {
                    tracing::warn!("Failed to send HoB backup script: {error:#}");
                    Some("Failed to send the backup script!".to_string())
                }
            },
            Err(error) => {
                tracing::warn!("Failed to build HoB backup script message: {error:#}");
                Some("Failed to send the backup script!".to_string())
            }
        }
    };

    let mut response_lines = vec![
        "## Sent Successfully".to_string(),
        format!(
            "The full HoB was sent to {} in the form of {message_count} messages.\n",
            channel.mention()
        ),
        deletion_feedback,
    ];
    if let Some(backup_feedback) = backup_feedback {
        response_lines.push(backup_feedback);
    }

    let container = CreateComponent::Container(
        CreateContainer::new(vec![CreateContainerComponent::TextDisplay(
            CreateTextDisplay::new(response_lines.join("\n")),
        )])
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

const MAX_HOB_BATCH_SPREAD_MILLIS: u64 = 60_000;

async fn delete_previous_hob_messages(
    http: &Http,
    channel: GenericChannelId,
    bot_id: UserId,
    before: MessageId,
) -> Result<usize> {
    let messages = channel
        .messages(http, GetMessages::new().before(before).limit(32))
        .await?;

    let Some(previous_message_ids) = previous_hob_batch_ids(&messages, bot_id) else {
        return Ok(0);
    };

    let deleted_count = previous_message_ids.len();
    for message_id in previous_message_ids {
        channel
            .delete_message(http, message_id, Some("Cleanup of previous HoB messages"))
            .await?;
    }

    Ok(deleted_count)
}

fn previous_hob_batch_ids(messages: &[Message], bot_id: UserId) -> Option<Vec<MessageId>> {
    let newest = messages.first()?;
    if !is_bot_hob_message(newest, bot_id) {
        return None;
    }

    let newest_timestamp = newest.timestamp.unix_timestamp_millis();
    let mut message_ids = Vec::new();

    for message in messages {
        if !is_bot_hob_message(message, bot_id)
            || message
                .timestamp
                .unix_timestamp_millis()
                .abs_diff(newest_timestamp)
                > MAX_HOB_BATCH_SPREAD_MILLIS
        {
            break;
        }

        message_ids.push(message.id);
    }

    Some(message_ids)
}

fn is_bot_hob_message(message: &Message, bot_id: UserId) -> bool {
    message.author.id == bot_id
        && message
            .flags
            .is_some_and(|flags| flags.contains(MessageFlags::IS_COMPONENTS_V2))
        && message
            .components
            .iter()
            .any(|component| matches!(component, Component::Container(_)))
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
