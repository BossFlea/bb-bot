use poise::serenity_prelude::{
    ButtonStyle, CreateButton, CreateComponent, CreateContainer, CreateContainerComponent,
    CreateSection, CreateSectionAccessory, CreateSectionComponent, CreateSeparator,
    CreateTextDisplay,
};

use crate::hob::{menu::SelectEntryState, types::HobEntry};
use crate::shared::menu::{
    MenuMessage,
    navigation::{self, PaginatedChunk},
};

const PAGE_SIZE: usize = 6;

pub fn generate_entry_list(
    menu_id: u64,
    hob_entries: &[HobEntry],
    session_state: &mut SelectEntryState,
) -> MenuMessage<'static> {
    let id_prefix = format!("hob:{menu_id}");

    let chunk = PaginatedChunk::new(hob_entries.len(), session_state.page, PAGE_SIZE);
    session_state.page = chunk.page;
    let hob_entries_paginated = &hob_entries[chunk.range.clone()];

    let divider = CreateContainerComponent::Separator(CreateSeparator::new(true));

    let entry_components: Vec<_> = hob_entries_paginated
        .iter()
        .map(|e| e.to_section_edit(&id_prefix))
        .flat_map(|section| [section, divider.clone()].into_iter())
        .collect();

    let showing = CreateSectionComponent::TextDisplay(CreateTextDisplay::new(format!(
        "Showing {}–{} of {} entries.",
        if chunk.total_pages == 0 {
            0
        } else {
            chunk.range.start + 1
        },
        chunk.range.end,
        hob_entries.len(),
    )));

    let title_section = match &session_state.search_query {
        Some(query) => {
            let title = CreateSectionComponent::TextDisplay(CreateTextDisplay::new(format!(
                "# Search Results: __`{query}`__"
            )));
            let reset_button = CreateSectionAccessory::Button(
                CreateButton::new(format!("{id_prefix}:reset_search"))
                    .emoji('❌')
                    .label("Reset Search")
                    .style(ButtonStyle::Secondary),
            );
            CreateContainerComponent::Section(CreateSection::new(vec![title], reset_button))
        }
        None => {
            let title =
                CreateSectionComponent::TextDisplay(CreateTextDisplay::new("# Manage HoB Entries"));
            let search_button = CreateSectionAccessory::Button(
                CreateButton::new(format!("{id_prefix}:search"))
                    .emoji('🔍')
                    .label("Search")
                    .style(ButtonStyle::Secondary),
            );
            CreateContainerComponent::Section(CreateSection::new(vec![title], search_button))
        }
    };

    let navigation = navigation::page_navigation_jump(&id_prefix, &chunk);

    let create_button = CreateButton::new(format!("{id_prefix}:create_entry"))
        .label("Create HoB Entry")
        .style(ButtonStyle::Success);

    let showing_section = CreateContainerComponent::Section(CreateSection::new(
        vec![showing],
        CreateSectionAccessory::Button(create_button),
    ));

    let components: Vec<_> = [title_section, showing_section, divider]
        .into_iter()
        .chain(entry_components)
        .chain([navigation])
        .collect();

    MenuMessage {
        components: vec![CreateComponent::Container(CreateContainer::new(components))],
    }
}
