use anyhow::{anyhow, Context as _, Result};
use poise::serenity_prelude::{
    small_fixed_array::FixedString, ButtonStyle, CacheHttp as _, ComponentInteraction,
    Context as SerenityContext, CreateButton, CreateComponent, CreateContainer,
    CreateInteractionResponse, CreateInteractionResponseMessage, CreateSection,
    CreateSectionAccessory, CreateSectionComponent, CreateTextDisplay, MessageFlags,
    ModalInteraction, ReactionType,
};

use crate::hob::{
    interaction::{modal, MessageEdit},
    menu::{HobEditState, SelectEntryState, ViewEntryState},
    types::{HobEntry, OneOffPlayers},
};
use crate::shared::{
    interaction::{modal as shared_modal, MenuChange},
    menu::navigation::GenerateMenu as _,
    types::Bingo,
    BotData,
};

pub async fn handle_component(
    ctx: &SerenityContext,
    interaction: &ComponentInteraction,
    mut action: impl Iterator<Item = &str>,
    menu_id: u64,
    session_state: &mut SelectEntryState,
) -> Result<MenuChange<'static, HobEditState>> {
    let db = &ctx.data::<BotData>().db_handle;
    let id_prefix = format!("hob:{menu_id}");

    match action.next().unwrap_or_default() {
        "goto_page" => {
            session_state.page = match action.next().unwrap_or_default() {
                "next" => session_state.page + 1,
                "prev" => session_state.page.saturating_sub(1),
                _ => 0,
            };

            Ok(MenuChange::message(MessageEdit::Interaction(
                session_state.generate(db, menu_id).await?,
            )))
        }
        "jump_page" => {
            let modal = shared_modal::JumpPage::create(&id_prefix);

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Modal(modal))
                .await?;
            Ok(MenuChange::none())
        }
        "search" => {
            let modal = shared_modal::Search::create_prefilled(
                &id_prefix,
                session_state.search_query.as_deref().unwrap_or("").into(),
            );

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Modal(modal))
                .await?;
            Ok(MenuChange::none())
        }
        "view_entry" => {
            let entry_id: u64 = action
                .next()
                .context("Invalid interaction: Expected additional argument")?
                .parse()
                .context("Invalid interaction: Expected entry ID")?;

            let mut new_state = ViewEntryState::new(entry_id, 0);
            let menu = new_state.generate(db, menu_id).await?;

            Ok(MenuChange::update_state(
                HobEditState::ViewEntry(new_state),
                MessageEdit::Interaction(menu),
            ))
        }
        "create_entry" => {
            let oneoff_text = CreateSectionComponent::TextDisplay(CreateTextDisplay::new(
                "## One-off Entry
Meant for one-off achievements by one or several players during a single bingo.",
            ));

            let ongoing_text = CreateSectionComponent::TextDisplay(CreateTextDisplay::new(
                "## Iterative Entry
Supports subentries, which each possess their own player, bingo and achieved 'value' fields. \
Meant for achievements that can be improved upon (e.g. Highest XY).",
            ));

            let oneoff_button = CreateButton::new(format!("{id_prefix}:create_oneoff_confirm"))
                .emoji(ReactionType::Unicode(FixedString::from_str_trunc("1️⃣")))
                .label("Create One-off Entry")
                .style(ButtonStyle::Success);

            let ongoing_button = CreateButton::new(format!("{id_prefix}:create_ongoing_confirm"))
                .emoji('🔁')
                .label("Create Iterative Entry")
                .style(ButtonStyle::Success);

            let oneoff_section = CreateComponent::Section(CreateSection::new(
                vec![oneoff_text],
                CreateSectionAccessory::Button(oneoff_button),
            ));

            let ongoing_section = CreateComponent::Section(CreateSection::new(
                vec![ongoing_text],
                CreateSectionAccessory::Button(ongoing_button),
            ));

            let container = CreateComponent::Container(CreateContainer::new(vec![
                CreateComponent::TextDisplay(CreateTextDisplay::new("# Choose HoB Entry Type")),
                oneoff_section,
                ongoing_section,
                CreateComponent::TextDisplay(CreateTextDisplay::new(
                    "-# All entries can be edited after creation.",
                )),
            ]));

            let message = CreateInteractionResponseMessage::new()
                .flags(MessageFlags::IS_COMPONENTS_V2)
                .components(vec![container])
                .ephemeral(true);

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Message(message))
                .await?;

            Ok(MenuChange::none())
        }
        "create_oneoff_confirm" => {
            let modal = modal::HobEntryOneoff::create(&id_prefix);

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Modal(modal))
                .await?;

            Ok(MenuChange::none())
        }
        "create_ongoing_confirm" => {
            let modal = modal::HobEntryOngoing::create(&id_prefix);

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Modal(modal))
                .await?;

            Ok(MenuChange::none())
        }
        "reset_search" => {
            session_state.page = 0;
            session_state.search_query = None;
            let menu = session_state.generate(db, menu_id).await?;
            Ok(MenuChange::message(MessageEdit::Interaction(menu)))
        }
        _ => Err(anyhow!("Invalid interaction: Unexpected action")),
    }
}

pub async fn handle_modal(
    ctx: &SerenityContext,
    interaction: &ModalInteraction,
    mut action: impl Iterator<Item = &str>,
    menu_id: u64,
    session_state: &mut SelectEntryState,
) -> Result<MenuChange<'static, HobEditState>> {
    let db = &ctx.data::<BotData>().db_handle;

    match action.next().unwrap_or_default() {
        "search_submit" => {
            let values = shared_modal::Search::validate(&interaction.data.components)?;

            if values.query.is_empty() {
                return Ok(MenuChange::none());
            };
            session_state.page = 0;
            session_state.search_query = Some(values.query.into_string());

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Acknowledge)
                .await?;
            let menu = session_state.generate(db, menu_id).await?;
            Ok(MenuChange::message(MessageEdit::Direct(menu)))
        }
        "jump_page_submit" => {
            let values = shared_modal::JumpPage::validate(&interaction.data.components)?;

            if values.page.is_empty() {
                return Ok(MenuChange::none());
            };
            let jump_page: usize = values.page.parse().unwrap_or(0);
            session_state.page = jump_page.saturating_sub(1);

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Acknowledge)
                .await?;
            let menu = session_state.generate(db, menu_id).await?;
            Ok(MenuChange::message(MessageEdit::Direct(menu)))
        }
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
            let entry_id = crate::shared::menu::generate_id();

            db.insert_hob_entry(HobEntry::OneOff {
                id: entry_id,
                title: values.title.into_string(),
                comment,
                bingo,
                players: OneOffPlayers { players },
            })
            .await?;

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Acknowledge)
                .await?;

            let mut new_state = ViewEntryState::new(entry_id, 0);
            let menu = new_state.generate(db, menu_id).await?;

            Ok(MenuChange::update_state(
                HobEditState::ViewEntry(new_state),
                MessageEdit::Direct(menu),
            ))
        }
        "ongoing_submit" => {
            let values = modal::HobEntryOngoing::validate(&interaction.data.components)?;

            let comment = Some(values.comment)
                .filter(|str| !str.trim().is_empty())
                .map(FixedString::into_string);
            let entry_id = crate::shared::menu::generate_id();

            db.insert_hob_entry(HobEntry::Ongoing {
                id: entry_id,
                title: values.title.into_string(),
                comment,
                subentries: Vec::new(),
            })
            .await?;

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Acknowledge)
                .await?;

            let mut new_state = ViewEntryState::new(entry_id, 0);
            let menu = new_state.generate(db, menu_id).await?;

            Ok(MenuChange::update_state(
                HobEditState::ViewEntry(new_state),
                MessageEdit::Direct(menu),
            ))
        }
        _ => Err(anyhow!("Invalid interaction: Unexpected action")),
    }
}
