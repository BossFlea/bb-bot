use anyhow::{Context as _, Result, anyhow};
use poise::{
    CreateReply, builtins,
    serenity_prelude::{
        CreateComponent, CreateContainer, CreateTextDisplay, GuildId, MessageFlags,
        colours::css::POSITIVE,
    },
};
use tracing::warn;

use crate::error::UserError;
use crate::shared::Context;

/// Register commands in a guild
#[poise::command(
    prefix_command,
    // NOTE: the normal permission requirements seems to always error on prefix commands
    // required_bot_permissions = "VIEW_CHANNEL | SEND_MESSAGES | READ_MESSAGE_HISTORY",
    // required_permissions = "MANAGE_GUILD",
    guild_only,
)]
pub async fn register(ctx: Context<'_>) -> Result<()> {
    let target_guild = ctx
        .guild_id()
        .context(UserError(anyhow!("Command must be run inside a guild")))?;
    let author = ctx
        .author_member()
        .await
        .context("Member missing in guild command invocation")?;

    let permissions = target_guild
        .to_partial_guild(ctx.http())
        .await?
        .member_permissions(&author);

    if !permissions.manage_guild() {
        warn!(
            "{} attempted to use the register command without permission",
            author.user.name
        );
        return Ok(());
    }

    let _typing = ctx.defer_or_broadcast().await?;

    // This is sometimes delayed due to rate limiting if executed in quick succession, which is the
    // reason for the deferring above
    builtins::register_in_guild(
        ctx.http(),
        &ctx.framework().options().commands,
        target_guild,
    )
    .await?;

    let commands_list = ctx
        .framework()
        .options()
        .commands
        .iter()
        .filter_map(|c| {
            c.slash_action
                .is_some()
                .then_some(format!("- `/{}`", c.qualified_name))
        })
        .collect::<Vec<_>>()
        .join("\n");

    let text = CreateComponent::TextDisplay(CreateTextDisplay::new(format!(
        "## Registered successfully
Successfully registered the following commands and their subcommands:\n{commands_list}
Note: All commands are visible only to users with the `MANAGE_GUILD` permission by default. \
**This is meant to be changed!** \
Role-based, per-command permissions should be set up manually \
in the server settings under `Integrations`.
-# The reason for this is that bots can only control command visibility by raw permissions, \
not by role. (endpoint exists but rejects bots)"
    )));

    let container =
        CreateComponent::Container(CreateContainer::new(vec![text]).accent_color(POSITIVE));

    ctx.send(
        CreateReply::new()
            .reply(true)
            .flags(MessageFlags::IS_COMPONENTS_V2)
            .components(vec![container]),
    )
    .await?;

    Ok(())
}

/// Unregister commands in a guild
#[poise::command(slash_command, guild_only, required_permissions = "MANAGE_GUILD")]
pub async fn unregister(
    ctx: Context<'_>,
    #[description = "Guild ID to unregister the commands"] guild: Option<GuildId>,
) -> Result<()> {
    let guild = guild.unwrap_or(
        ctx.guild_id()
            .context(UserError(anyhow!("Command must be run inside a guild")))?,
    );

    guild.set_commands(ctx.http(), &[]).await?;

    let container = CreateComponent::Container(
        CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
            "## Unregistered Successfully
Unregistered this bot's commands from the current guild.",
        ))])
        .accent_color(POSITIVE),
    );

    ctx.send(
        CreateReply::new()
            .flags(MessageFlags::IS_COMPONENTS_V2)
            .components(vec![container])
            .ephemeral(true),
    )
    .await?;

    Ok(())
}
