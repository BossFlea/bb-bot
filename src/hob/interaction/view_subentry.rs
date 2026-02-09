use anyhow::{Context as _, Result, anyhow};
use poise::serenity_prelude::{
    ButtonStyle, CacheHttp as _, ComponentInteraction, Context as SerenityContext, CreateButton,
    CreateComponent, CreateContainer, CreateContainerComponent, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateSection, CreateSectionAccessory,
    CreateSectionComponent, CreateSeparator, CreateTextDisplay, MessageFlags, ModalInteraction,
    colours::css::{DANGER, POSITIVE},
};

use crate::hob::{
    db::{DeleteHobSubentry, GetHobSubentry, UpdateHobSubentry},
    interaction::{MessageEdit, modal},
    menu::{HobEditState, ViewEntryState, ViewSubentryState},
    types::OngoingSubentry,
};
use crate::shared::{
    BotData,
    interaction::MenuChange,
    menu::{
        ACCENT_COLOR,
        navigation::{BacktrackState as _, GenerateMenu as _},
    },
    types::Bingo,
};

pub async fn handle_component(
    ctx: &SerenityContext,
    interaction: &ComponentInteraction,
    mut action: impl Iterator<Item = &str>,
    menu_id: u64,
    session_state: &mut ViewSubentryState,
) -> Result<MenuChange<'static, HobEditState>> {
    let db = &ctx.data::<BotData>().db_handle;
    let id_prefix = format!("hob:{menu_id}");

    match action.next().unwrap_or_default() {
        "back" => {
            let entry_id = session_state.entry_id;
            let mut new_state = session_state
                .take_referrer_or(|| HobEditState::ViewEntry(ViewEntryState::new(entry_id, 0)));
            let menu = new_state.generate(db, menu_id).await?;
            Ok(MenuChange::new(new_state, MessageEdit::Interaction(menu)))
        }
        "preview" => {
            let subentry = db
                .request(GetHobSubentry {
                    id: session_state.id,
                    entry_id: session_state.entry_id,
                })
                .await??
                .context("Invalid subentry ID")?;
            let subentry_text = CreateContainerComponent::TextDisplay(CreateTextDisplay::new(
                subentry.to_list_item(),
            ));
            let title = CreateContainerComponent::TextDisplay(CreateTextDisplay::new(
                "### *Isolated Subentry Preview*",
            ));

            let divider = CreateContainerComponent::Separator(CreateSeparator::new(true));

            let container = CreateComponent::Container(
                CreateContainer::new(vec![title, divider.clone(), subentry_text, divider])
                    .accent_color(ACCENT_COLOR),
            );

            let message = CreateInteractionResponseMessage::new()
                .flags(MessageFlags::IS_COMPONENTS_V2)
                .components(vec![container])
                .ephemeral(true);

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Message(message))
                .await?;
            Ok(MenuChange::none())
        }
        "edit" => {
            let subentry = db
                .request(GetHobSubentry {
                    id: session_state.id,
                    entry_id: session_state.entry_id,
                })
                .await??
                .context("Invalid subentry ID")?;

            let modal = modal::HobOngoingSubentry::create_prefilled(
                &id_prefix,
                subentry.player.into(),
                subentry.value.into(),
                subentry.bingo.to_short_string().into(),
            );

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Modal(modal))
                .await?;
            Ok(MenuChange::none())
        }
        "delete" => {
            let confirm_button = CreateButton::new(format!("{id_prefix}:delete_confirm"))
                .label("Delete")
                .style(ButtonStyle::Danger);

            let confirm_section = CreateContainerComponent::Section(CreateSection::new(
                vec![CreateSectionComponent::TextDisplay(CreateTextDisplay::new(
                    "## Confirm Deletion\nAre you sure you want to delete this subentry?",
                ))],
                CreateSectionAccessory::Button(confirm_button),
            ));

            let container = CreateComponent::Container(
                CreateContainer::new(vec![confirm_section]).accent_color(DANGER),
            );

            let message = CreateInteractionResponseMessage::new()
                .flags(MessageFlags::IS_COMPONENTS_V2)
                .components(vec![container])
                .ephemeral(true);

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Message(message))
                .await?;
            Ok(MenuChange::none())
        }
        "delete_confirm" => {
            db.request(DeleteHobSubentry {
                id: session_state.id,
                entry_id: session_state.entry_id,
            })
            .await??;

            let success_text = CreateContainerComponent::TextDisplay(CreateTextDisplay::new(
                "## Deleted Successfully\nThe subentry was successfully removed from the database.",
            ));

            let container = CreateComponent::Container(
                CreateContainer::new(vec![success_text]).accent_color(POSITIVE),
            );

            let message = CreateInteractionResponseMessage::new()
                .flags(MessageFlags::IS_COMPONENTS_V2)
                .components(vec![container])
                .ephemeral(true);

            let entry_id = session_state.entry_id;
            let mut new_state = session_state
                .take_referrer_or(|| HobEditState::ViewEntry(ViewEntryState::new(entry_id, 0)));

            interaction
                .create_response(
                    ctx.http(),
                    CreateInteractionResponse::UpdateMessage(message),
                )
                .await?;

            let menu = new_state.generate(db, menu_id).await?;
            Ok(MenuChange::new(new_state, MessageEdit::Direct(menu)))
        }
        _ => Err(anyhow!("Invalid interaction: Unexpected action")),
    }
}

pub async fn handle_modal(
    ctx: &SerenityContext,
    interaction: &ModalInteraction,
    mut action: impl Iterator<Item = &str>,
    menu_id: u64,
    session_state: &mut ViewSubentryState,
) -> Result<MenuChange<'static, HobEditState>> {
    let db = &ctx.data::<BotData>().db_handle;

    match action.next().unwrap_or_default() {
        "subentry_submit" => {
            let values = modal::HobOngoingSubentry::validate(&interaction.data.components)?;

            let bingo = Bingo::from_input(&values.bingo)?;

            db.request(UpdateHobSubentry {
                subentry: OngoingSubentry {
                    id: session_state.id,
                    entry_id: session_state.entry_id,
                    player: values.player.into_string(),
                    value: values.value.into_string(),
                    bingo,
                },
            })
            .await??;

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Acknowledge)
                .await?;

            let menu = session_state.generate(db, menu_id).await?;
            Ok(MenuChange::message(MessageEdit::Direct(menu)))
        }
        _ => Err(anyhow!("Invalid interaction: Unexpected action")),
    }
}
