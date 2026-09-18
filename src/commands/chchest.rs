use anyhow::{Result, anyhow, bail};
use poise::serenity_prelude::CreateInteractionResponse;

use crate::chchest::modal::ChChestReport;
use crate::error::UserError;
use crate::shared::Context;

/// Report a Crystal Hollows find (robot part chest, key guardian, or other loot)
#[poise::command(
    slash_command,
    manual_cooldowns = true,
    required_bot_permissions = "VIEW_CHANNEL | SEND_MESSAGES"
)]
pub async fn chchest(ctx: Context<'_>) -> Result<()> {
    let Context::Application(app_ctx) = ctx else {
        bail!(UserError(anyhow!(
            "This command can only be used as a slash command"
        )))
    };
    let modal = ChChestReport::create("chchest");
    app_ctx
        .interaction
        .create_response(ctx.http(), CreateInteractionResponse::Modal(modal))
        .await?;
    Ok(())
}
