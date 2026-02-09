use anyhow::{Context as _, Result, anyhow};
use poise::{
    CreateReply,
    serenity_prelude::{
        CreateAllowedMentions, CreateComponent, CreateContainer, CreateContainerComponent,
        CreateSection, CreateSectionAccessory, CreateSectionComponent, CreateTextDisplay,
        CreateThumbnail, CreateUnfurledMediaItem, Mentionable as _, MessageFlags, User,
    },
};

use crate::error::UserError;
use crate::shared::Context;

/// View the reason a user was banned.
#[poise::command(slash_command, required_bot_permissions = "BAN_MEMBERS")]
pub async fn baninfo(
    ctx: Context<'_>,
    #[description = "Member or User ID"] user: User,
) -> Result<()> {
    let guild = ctx
        .guild_id()
        .context(UserError(anyhow!("Command invoked outside of a guild")))?;

    let ban = guild.get_ban(ctx.http(), user.id).await?;

    let ban_details = if let Some(ban) = ban {
        if let Some(reason) = ban.reason {
            format!("### Ban Reason\n{reason}")
        } else {
            "### Ban Reason\n*No reason provided.*".to_string()
        }
    } else {
        "### No active ban.".to_string()
    };

    let info_text = format!(
        "## Ban Information
### User {}
Username: `{}`
User ID: `{}`
{}",
        user.mention(),
        user.name,
        user.id,
        ban_details
    );

    let container = CreateComponent::Container(CreateContainer::new(vec![
        CreateContainerComponent::Section(CreateSection::new(
            vec![CreateSectionComponent::TextDisplay(CreateTextDisplay::new(
                info_text,
            ))],
            CreateSectionAccessory::Thumbnail(CreateThumbnail::new(CreateUnfurledMediaItem::new(
                user.face(),
            ))),
        )),
    ]));

    ctx.send(
        CreateReply::new()
            .flags(MessageFlags::IS_COMPONENTS_V2)
            .allowed_mentions(CreateAllowedMentions::new())
            .components(vec![container]),
    )
    .await?;

    Ok(())
}
