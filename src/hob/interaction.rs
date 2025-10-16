use std::sync::Arc;

use anyhow::{anyhow, Context as _, Result};
use either::Either;
use poise::serenity_prelude::{
    colours::css::DANGER, CacheHttp as _, ComponentInteraction, Context as SerenityContext,
    CreateComponent, CreateContainer, CreateInteractionResponse, CreateInteractionResponseMessage,
    CreateTextDisplay, Mentionable as _, MessageFlags, ModalInteraction,
};
use tokio::sync::MutexGuard;
use tracing::{info, warn};

use crate::error::UserError;
use crate::hob::menu::{HobEditSession, HobEditState};
use crate::shared::{interaction::MessageEdit, menu::navigation::Backtrack as _, BotData};

mod modal;
mod select_entry;
mod view_entry;
mod view_subentry;

pub async fn handle_interaction(
    ctx: &SerenityContext,
    interaction: Either<&ComponentInteraction, &ModalInteraction>,
    mut action: impl Iterator<Item = &str>,
) -> Result<()> {
    match &interaction {
        Either::Left(component_interaction) => {
            info!(
                "{} triggered component interaction: '{}'",
                component_interaction.user.name, component_interaction.data.custom_id
            );
        }
        Either::Right(modal_interaction) => {
            info!(
                "{} triggered modal interaction: '{}'",
                modal_interaction.user.name, modal_interaction.data.custom_id
            );
        }
    }

    let menu_id = action
        .next()
        .unwrap_or_default()
        .parse::<u64>()
        .context("Invalid interaction: Expected menu ID")?;

    // NOTE: lock dropped at the end of the expression
    let session_mutex = Arc::clone(
        ctx.data::<BotData>()
            .hob_sessions
            .lock()
            .await
            .get(&menu_id)
            .context(UserError(anyhow!("This menu has expired!")))?,
    );

    let mut session = session_mutex.lock().await;

    if let Either::Left(component_interaction) = &interaction
        && component_interaction.user.id != session.owner.id
    {
        warn!(
            "{} tried to interact with {}'s menu",
            component_interaction.user.name, session.owner.name
        );

        let container = CreateComponent::Container(
            CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
                format!(
                    "## You don't own this menu!
Only {} is allowed to interact with this menu.",
                    session.owner.mention()
                ),
            ))])
            .accent_color(DANGER),
        );

        let message = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::default()
                .flags(MessageFlags::IS_COMPONENTS_V2)
                .components(vec![container])
                .ephemeral(true),
        );

        component_interaction
            .create_response(ctx.http(), message)
            .await?;
        return Ok(());
    }

    session.timeout_reset.notify_one();

    let new_content = match interaction {
        Either::Left(component_interaction) => {
            component(ctx, component_interaction, action, &mut session).await?
        }
        Either::Right(modal_interaction) => {
            modal(ctx, modal_interaction, action, &mut session).await?
        }
    };

    match new_content {
        MessageEdit::Interaction(menu) => {
            interaction
                .left()
                .context("Invalid edit method for modal interaction")?
                .create_response(
                    ctx.http(),
                    CreateInteractionResponse::UpdateMessage(menu.into_interaction_response()),
                )
                .await?
        }
        MessageEdit::Direct(menu) => {
            ctx.http()
                .edit_message(
                    session.channel_id,
                    session.message_id,
                    &menu.into_edit(),
                    vec![],
                )
                .await?;
        }
        MessageEdit::NoEdit => (),
    }

    Ok(())
}

async fn component(
    ctx: &SerenityContext,
    interaction: &ComponentInteraction,
    action: impl Iterator<Item = &str>,
    session: &mut MutexGuard<'_, HobEditSession>,
) -> Result<MessageEdit<'static>> {
    let menu_id = session.menu_id;
    let change = match &mut session.state {
        HobEditState::SelectEntry(state) => {
            select_entry::handle_component(ctx, interaction, action, menu_id, state).await
        }
        HobEditState::ViewEntry(state) => {
            view_entry::handle_component(ctx, interaction, action, menu_id, state).await
        }
        HobEditState::ViewSubentry(state) => {
            view_subentry::handle_component(ctx, interaction, action, menu_id, state).await
        }
    }?;
    if let Some(state) = change.new_state {
        if change.update_referrer_state {
            session.state.update_state(state);
        } else {
            session.state = state
        }
    }
    Ok(change.message)
}

async fn modal(
    ctx: &SerenityContext,
    interaction: &ModalInteraction,
    action: impl Iterator<Item = &str>,
    session: &mut MutexGuard<'_, HobEditSession>,
) -> Result<MessageEdit<'static>> {
    let menu_id = session.menu_id;
    let change = match &mut session.state {
        HobEditState::SelectEntry(state) => {
            select_entry::handle_modal(ctx, interaction, action, menu_id, state).await
        }
        HobEditState::ViewEntry(state) => {
            view_entry::handle_modal(ctx, interaction, action, menu_id, state).await
        }
        HobEditState::ViewSubentry(state) => {
            view_subentry::handle_modal(ctx, interaction, action, menu_id, state).await
        }
    }?;
    if let Some(state) = change.new_state {
        if change.update_referrer_state {
            session.state.update_state(state);
        } else {
            session.state = state
        }
    }
    Ok(change.message)
}
