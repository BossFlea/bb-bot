use poise::serenity_prelude::{
    ButtonStyle, CreateActionRow, CreateButton, CreateComponent, CreateContainer, CreateSection,
    CreateSectionAccessory, CreateSectionComponent, CreateSeparator, CreateTextDisplay,
};

use crate::hob::types::{HobEntry, OngoingSubentry};
use crate::shared::menu::{
    MenuMessage,
    navigation::{self, PaginatedChunk},
};

const SUBENTRIES_PAGE_SIZE: usize = 5;

pub fn generate_entry(menu_id: u64, hob_entry: HobEntry, page: &mut usize) -> MenuMessage<'static> {
    let id_prefix = format!("hob:{menu_id}");
    let title = CreateSectionComponent::TextDisplay(CreateTextDisplay::new("# View HoB Entry"));

    let delete_button = CreateSectionAccessory::Button(
        CreateButton::new(format!("{id_prefix}:delete"))
            .label("Delete")
            .style(ButtonStyle::Danger),
    );
    let edit_button = CreateButton::new(format!("{id_prefix}:edit"))
        .emoji('✏')
        .style(ButtonStyle::Primary);
    let preview_button = CreateButton::new(format!("{id_prefix}:preview"))
        .label("Preview")
        .style(ButtonStyle::Secondary);

    let title_section = CreateComponent::Section(CreateSection::new(vec![title], delete_button));

    let edit_row = CreateComponent::ActionRow(CreateActionRow::Buttons(
        vec![edit_button, preview_button].into(),
    ));

    let divider = CreateComponent::Separator(CreateSeparator::new(true));

    let components = match hob_entry {
        HobEntry::OneOff {
            title,
            comment,
            bingo,
            players,
            ..
        } => {
            let description = CreateComponent::TextDisplay(CreateTextDisplay::new(format!(
                "
### Type\nOne-off achievement
### Title\n{}
### Players\n{}
### Bingo\n{}
### Comment\n{}
",
                title,
                players.to_list(),
                bingo,
                comment
                    .filter(|s| !s.is_empty())
                    .unwrap_or("*None*".to_string())
            )));

            let navigation_row = navigation::nagivation_back(&id_prefix);

            vec![
                title_section,
                edit_row,
                description,
                divider,
                navigation_row,
            ]
        }
        HobEntry::Ongoing {
            title,
            comment,
            subentries,
            ..
        } => {
            let page_chunk = PaginatedChunk::new(subentries.len(), *page, SUBENTRIES_PAGE_SIZE);
            *page = page_chunk.page;
            let subentries_paginated = &subentries[page_chunk.range.clone()];

            let description = CreateComponent::TextDisplay(CreateTextDisplay::new(format!(
                "
### Type\nIterative achievement
### Title\n{}
### Comment\n{}
",
                title,
                comment
                    .filter(|s| !s.is_empty())
                    .unwrap_or("*None*".to_string()),
            )));

            let subentry_text =
                CreateSectionComponent::TextDisplay(CreateTextDisplay::new(format!(
                    "## Subentries\n{}",
                    if subentries_paginated.is_empty() {
                        "*None*"
                    } else {
                        ""
                    }
                )));
            let create_button = CreateSectionAccessory::Button(
                CreateButton::new(format!("{}:create_subentry", id_prefix))
                    .label("Create Subentry")
                    .style(ButtonStyle::Success),
            );
            let subentry_section =
                CreateComponent::Section(CreateSection::new(vec![subentry_text], create_button));

            let subentries = subentries_paginated
                .iter()
                .flat_map(|s| [s.to_section(&id_prefix), divider.clone()]);

            let navigation_row = navigation::page_navigation_subentry(&id_prefix, &page_chunk);

            vec![
                title_section,
                edit_row,
                description,
                subentry_section,
                divider.clone(),
            ]
            .into_iter()
            .chain(subentries)
            .chain([navigation_row])
            .collect()
        }
    };

    let container = CreateComponent::Container(CreateContainer::new(components));

    MenuMessage {
        components: vec![container],
    }
}

pub fn generate_subentry(menu_id: u64, subentry: OngoingSubentry) -> MenuMessage<'static> {
    let id_prefix = format!("hob:{menu_id}");
    let title = CreateSectionComponent::TextDisplay(CreateTextDisplay::new("# View Subentry"));

    let delete_button = CreateSectionAccessory::Button(
        CreateButton::new(format!("{id_prefix}:delete"))
            .label("Delete")
            .style(ButtonStyle::Danger),
    );
    let edit_button = CreateButton::new(format!("{id_prefix}:edit"))
        .emoji('✏')
        .style(ButtonStyle::Primary);

    let preview_button = CreateButton::new(format!("{id_prefix}:preview"))
        .label("Preview")
        .style(ButtonStyle::Secondary);

    let title_section = CreateComponent::Section(CreateSection::new(vec![title], delete_button));

    let edit_row = CreateComponent::ActionRow(CreateActionRow::Buttons(
        vec![edit_button, preview_button].into(),
    ));

    let divider = CreateComponent::Separator(CreateSeparator::new(true));

    let description = CreateComponent::TextDisplay(CreateTextDisplay::new(format!(
        "
### Type\nOne-off achievement
### Player\n`{}`
### Value\n{}
### Bingo\n{}
",
        subentry.player, subentry.value, subentry.bingo,
    )));

    let navigation_row = navigation::nagivation_back(&id_prefix);

    let components = vec![
        title_section,
        edit_row,
        description,
        divider,
        navigation_row,
    ];

    let container = CreateComponent::Container(CreateContainer::new(components));

    MenuMessage {
        components: vec![container],
    }
}
