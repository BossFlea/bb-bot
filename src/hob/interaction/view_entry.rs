use anyhow::{Context as _, Result, anyhow};
use poise::serenity_prelude::{
    ButtonStyle, CacheHttp as _, ComponentInteraction, Context as SerenityContext, CreateButton,
    CreateComponent, CreateContainer, CreateContainerComponent, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateSection, CreateSectionAccessory,
    CreateSectionComponent, CreateSeparator, CreateTextDisplay, MessageFlags, ModalInteraction,
    colours::css::{DANGER, POSITIVE},
    small_fixed_array::FixedString,
};

use crate::hob::{
    db::{DeleteHobEntry, GetHobEntry, InsertHobSubentry, UpdateHobEntry},
    interaction::{MessageEdit, modal},
    menu::{HobEditState, SelectEntryState, ViewEntryState, ViewSubentryState},
    types::{HobEntry, OneOffPlayers, OngoingSubentry},
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
    session_state: &mut ViewEntryState,
) -> Result<MenuChange<'static, HobEditState>> {
    let db = &ctx.data::<BotData>().db_handle;
    let id_prefix = format!("hob:{menu_id}");

    match action.next().unwrap_or_default() {
        "back" => {
            let mut new_state = session_state
                .take_referrer_or(|| HobEditState::SelectEntry(SelectEntryState::new(0, None)));

            let menu = new_state.generate(db, menu_id).await?;
            Ok(MenuChange::new(new_state, MessageEdit::Interaction(menu)))
        }
        "goto_page" => {
            session_state.page = match action.next().unwrap_or_default() {
                "next" => session_state.page + 1,
                "prev" => session_state.page.saturating_sub(1),
                _ => 0,
            };
            let menu = session_state.generate(db, menu_id).await?;
            Ok(MenuChange::message(MessageEdit::Interaction(menu)))
        }
        "preview" => {
            let entry = db
                .request(GetHobEntry {
                    id: session_state.id,
                })
                .await??
                .context("Invalid entry ID")?;
            let entry_text = entry.to_text_display().0;
            let title = CreateContainerComponent::TextDisplay(CreateTextDisplay::new(
                "### *Single Entry Preview*",
            ));

            let divider = CreateContainerComponent::Separator(CreateSeparator::new(true));
            let container = CreateComponent::Container(
                CreateContainer::new(vec![title, divider.clone(), entry_text, divider])
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
            let hob_entry = db
                .request(GetHobEntry {
                    id: session_state.id,
                })
                .await??
                .context("Invalid entry ID")?;

            let modal = {
                match hob_entry {
                    HobEntry::OneOff {
                        title,
                        comment,
                        bingo,
                        players,
                        ..
                    } => modal::HobEntryOneoff::create_prefilled(
                        &id_prefix,
                        title.into(),
                        players.to_plain_list().into(),
                        bingo.to_short_string().into(),
                        comment.unwrap_or_default().into(),
                    ),
                    HobEntry::Ongoing { title, comment, .. } => {
                        modal::HobEntryOngoing::create_prefilled(
                            &id_prefix,
                            title.into(),
                            comment.unwrap_or_default().into(),
                        )
                    }
                }
            };

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
                    "## Confirm Deletion\nAre you sure you want to delete this HoB entry?",
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
            db.request(DeleteHobEntry {
                id: session_state.id,
            })
            .await??;

            let success_text = CreateContainerComponent::TextDisplay(CreateTextDisplay::new(
                "## Deleted Successfully\nThe entry was successfully removed from the database.",
            ));

            let container = CreateComponent::Container(
                CreateContainer::new(vec![success_text]).accent_color(POSITIVE),
            );

            let message = CreateInteractionResponseMessage::new()
                .flags(MessageFlags::IS_COMPONENTS_V2)
                .components(vec![container])
                .ephemeral(true);

            let mut new_state = session_state
                .take_referrer_or(|| HobEditState::SelectEntry(SelectEntryState::new(0, None)));

            interaction
                .create_response(
                    ctx.http(),
                    CreateInteractionResponse::UpdateMessage(message),
                )
                .await?;

            let menu = new_state.generate(db, menu_id).await?;
            Ok(MenuChange::new(new_state, MessageEdit::Direct(menu)))
        }
        "view_subentry" => {
            let subentry_id: u64 = action
                .next()
                .context("Invalid interaction: Expected additional argument")?
                .parse()
                .context("Invalid interaction: Expected subentry ID")?;

            let mut new_state =
                HobEditState::ViewSubentry(ViewSubentryState::new(subentry_id, session_state.id));

            let menu = new_state.generate(db, menu_id).await?;
            Ok(MenuChange::update_state(
                new_state,
                MessageEdit::Interaction(menu),
            ))
        }
        "create_subentry" => {
            let modal = modal::HobOngoingSubentry::create(&id_prefix);

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Modal(modal))
                .await?;

            Ok(MenuChange::none())
        }
        _ => Err(anyhow!("Invalid interaction: Unexpected action")),
    }
}

pub async fn handle_modal(
    ctx: &SerenityContext,
    interaction: &ModalInteraction,
    mut action: impl Iterator<Item = &str>,
    menu_id: u64,
    session_state: &mut ViewEntryState,
) -> Result<MenuChange<'static, HobEditState>> {
    let db = &ctx.data::<BotData>().db_handle;

    match action.next().unwrap_or_default() {
        "oneoff_submit" => {
            let values = modal::HobEntryOneoff::validate(&interaction.data.components)?;

            let players: Vec<String> = values
                .players
                .split(",")
                .map(str::trim)
                .filter(|str| !str.is_empty())
                .map(String::from)
                .collect();

            let bingo = Bingo::from_input(&values.bingo)?;

            let comment = Some(values.comment)
                .filter(|str| !str.trim().is_empty())
                .map(FixedString::into_string);

            db.request(UpdateHobEntry {
                entry: HobEntry::OneOff {
                    id: session_state.id,
                    title: values.title.into_string(),
                    comment,
                    bingo,
                    players: OneOffPlayers { players },
                },
            })
            .await??;

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Acknowledge)
                .await?;

            let menu = session_state.generate(db, menu_id).await?;
            Ok(MenuChange::message(MessageEdit::Direct(menu)))
        }
        "ongoing_submit" => {
            let values = modal::HobEntryOngoing::validate(&interaction.data.components)?;

            let comment = Some(values.comment)
                .filter(|str| !str.trim().is_empty())
                .map(FixedString::into_string);

            db.request(UpdateHobEntry {
                entry: HobEntry::Ongoing {
                    id: session_state.id,
                    title: values.title.into_string(),
                    comment,
                    subentries: Vec::new(), // edited separately
                },
            })
            .await??;

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Acknowledge)
                .await?;

            let menu = session_state.generate(db, menu_id).await?;
            Ok(MenuChange::message(MessageEdit::Direct(menu)))
        }
        "subentry_submit" => {
            let values = modal::HobOngoingSubentry::validate(&interaction.data.components)?;

            let bingo = Bingo::from_input(&values.bingo)?;
            let subentry_id = crate::shared::menu::generate_id();

            db.request(InsertHobSubentry {
                subentry: OngoingSubentry {
                    id: subentry_id,
                    entry_id: session_state.id,
                    player: values.player.into_string(),
                    value: values.value.into_string(),
                    bingo,
                },
                ongoing_entry_id: session_state.id,
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
