use anyhow::Result;

use crate::shared::Context;
use crate::splashes::splashlist;

/// Create and send the splashlist
#[poise::command(
    slash_command,
    required_bot_permissions = "VIEW_CHANNEL | SEND_MESSAGES"
)]
pub async fn splashlist(
    ctx: Context<'_>,
    #[description = "Send the splash list as an ephemeral message (for prior inspection)"]
    ephemeral: Option<bool>,
) -> Result<()> {
    let ephemeral = ephemeral.unwrap_or(false);

    if ephemeral {
        ctx.defer_ephemeral().await?;
    } else {
        ctx.defer().await?;
    }

    let message = splashlist::generate_message(&ctx).await?;
    ctx.send(message.ephemeral(ephemeral)).await?;

    Ok(())
}
