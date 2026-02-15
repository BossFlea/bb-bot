use std::sync::LazyLock;

use anyhow::{Context as _, Result, anyhow};
use poise::{
    CreateReply,
    serenity_prelude::{
        CreateAllowedMentions, CreateComponent, CreateContainer, CreateContainerComponent,
        CreateTextDisplay, EmojiId, Mentionable as _, MessageFlags, colours::css::POSITIVE,
    },
};
use regex::Regex;

use crate::config::{SPLASH_REMINDER_CHANNEL, SPLASH_REMINDER_ROLE};
use crate::error::UserError;
use crate::shared::{Context, db::SetSplashReminder};

static EMOJI_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^<a?:.+:(\d+)>$").unwrap());

/// Manage splash reminders sent after 1 hour or a number of reactions on the latest splash
#[poise::command(
    slash_command,
    required_bot_permissions = "VIEW_CHANNEL | SEND_MESSAGES | READ_MESSAGE_HISTORY"
)]
pub async fn splashreminder(
    ctx: Context<'_>,
    #[description = "Whether to enable splash reminders"] enable: bool,
    #[description = "Emoji to use for reaction count; must be a server-specific emoji (feature disabled if omitted)"]
    emoji: Option<String>,
    #[description = "Minimum reaction count to send reminder (defaults to 50)"]
    reaction_count: Option<u32>,
) -> Result<()> {
    let emoji_id = if let Some(emoji) = &emoji {
        let captures = EMOJI_REGEX.captures(emoji).ok_or_else(|| {
            UserError(anyhow!(
                "`emoji` must contain a single, server-specific emoji"
            ))
        })?;
        let id: u64 = captures
            .get(1)
            .expect("first capture group should always exist")
            .as_str()
            .parse()
            .context(UserError(anyhow!("Invalid emoji ID")))?;
        Some(EmojiId::new(id))
    } else {
        None
    };

    ctx.data()
        .db_handle
        .request(SetSplashReminder {
            enabled: enable,
            emoji: emoji_id,
            emoji_count: reaction_count,
        })
        .await??;

    let text = CreateContainerComponent::TextDisplay(CreateTextDisplay::new(if enable {
        Cow::Owned(format!(
            "## Enabled Splash Reminders
{} will be pinged in {} when there hasn't been a splash for 1h during a bingo{}.",
            SPLASH_REMINDER_ROLE.mention(),
            SPLASH_REMINDER_CHANNEL.mention(),
            if let Some(emoji_mention) = emoji {
                format!(
                    ", or when the latest splash message has {}+ {emoji_mention} reactions",
                    reaction_count.unwrap_or(50)
                )
            } else {
                String::new()
            }
        ))
    } else {
        Cow::Borrowed(
            "## Disabled Splash Reminders
Splash reminders will no longer be sent.",
        )
    }));

    ctx.send(
        CreateReply::new()
            .flags(MessageFlags::IS_COMPONENTS_V2)
            .allowed_mentions(CreateAllowedMentions::new())
            .components(vec![CreateComponent::Container(
                CreateContainer::new(vec![text]).accent_color(POSITIVE),
            )]),
    )
    .await?;

    Ok(())
}
