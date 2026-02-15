use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use poise::serenity_prelude::{
    CreateAllowedMentions, CreateComponent, CreateContainer, CreateContainerComponent,
    CreateMessage, CreateTextDisplay, Http, Mentionable, MessageFlags, colours::css::DANGER,
};
use tokio::{select, sync::oneshot};
use tracing::error;

use crate::config::{SPLASH_REMINDER_CHANNEL, SPLASH_REMINDER_ROLE};

pub enum ReminderVariant {
    Time,
    Reactions {
        emoji_mention: String,
        emoji_count: u32,
    },
}

pub async fn send_reminder(http: Arc<Http>, variant: ReminderVariant) -> Result<()> {
    let variant_text = match variant {
        ReminderVariant::Time => "It has been 1 hour since the last splash!".to_string(),
        ReminderVariant::Reactions {
            emoji_mention,
            emoji_count,
        } => {
            format!("The latest splash message has {emoji_count}+ {emoji_mention} reactions!",)
        }
    };

    let text = CreateContainerComponent::TextDisplay(CreateTextDisplay::new(format!(
        "## Splash Needed
{variant_text}
{}",
        SPLASH_REMINDER_ROLE.mention()
    )));

    SPLASH_REMINDER_CHANNEL
        .send_message(
            &http,
            CreateMessage::new()
                .flags(MessageFlags::IS_COMPONENTS_V2)
                .allowed_mentions(CreateAllowedMentions::new().roles(&[SPLASH_REMINDER_ROLE]))
                .components(vec![CreateComponent::Container(
                    CreateContainer::new(vec![text]).accent_color(DANGER),
                )]),
        )
        .await?;

    Ok(())
}

pub const TIMER_WAIT_SECS: u64 = 3600;

pub async fn spawn_timer(http: Arc<Http>, cancel_rx: oneshot::Receiver<()>) {
    println!("spawned timer");
    tokio::spawn(async move {
        select! {
            _ = tokio::time::sleep(Duration::from_secs(TIMER_WAIT_SECS)) => (),
            _ = cancel_rx => return,
        };

        if let Err(err) = send_reminder(http, ReminderVariant::Time).await {
            error!("Failed to send splash reminder: {err:#}");
        };
    });
}
