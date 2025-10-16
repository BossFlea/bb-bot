use anyhow::{anyhow, Result};
use either::Either;
use poise::serenity_prelude::{ComponentInteraction, Context as SerenityContext, ModalInteraction};

mod config;
mod modal;
mod request;

pub async fn handle_interaction(
    ctx: &SerenityContext,
    interaction: Either<&ComponentInteraction, &ModalInteraction>,
    mut action: impl Iterator<Item = &str>,
) -> Result<()> {
    match action.next().unwrap_or_default() {
        "request" => request::handle_interaction(ctx, interaction, action).await,
        "config" => config::handle_interaction(ctx, interaction, action).await,
        _ => Err(anyhow!("Invalid interaction: Unknown subcategory")),
    }
}
