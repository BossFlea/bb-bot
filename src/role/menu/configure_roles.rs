use poise::serenity_prelude::{
    ButtonStyle, CreateActionRow, CreateButton, CreateComponent, CreateContainer, CreateSection,
    CreateSectionAccessory, CreateSectionComponent, CreateSelectMenu, CreateSelectMenuOption,
    CreateSeparator, CreateTextDisplay,
};

use crate::role::types::RoleMappingKindRaw;
use crate::role::{menu::RoleConfigState, types::RoleMapping};
use crate::shared::menu::{
    MenuMessage,
    navigation::{self, PaginatedChunk},
};

const PAGE_SIZE: usize = 6;

pub fn generate(
    menu_id: u64,
    role_mappings: &[RoleMapping],
    session_state: &mut RoleConfigState,
) -> MenuMessage<'static> {
    let id_prefix = format!("role:config:{menu_id}");

    let chunk = PaginatedChunk::new(role_mappings.len(), session_state.page, PAGE_SIZE);
    session_state.page = chunk.page;
    let role_mappings_paginated = &role_mappings[chunk.range.clone()];

    let divider = CreateComponent::Separator(CreateSeparator::new(true));

    let role_components: Vec<_> = role_mappings_paginated
        .iter()
        .map(|r| r.to_section_delete(&id_prefix))
        .collect();

    let title_section = CreateComponent::Section(CreateSection::new(
        vec![CreateSectionComponent::TextDisplay(CreateTextDisplay::new(
            "# Configure Role Bindings",
        ))],
        CreateSectionAccessory::Button(
            CreateButton::new(format!("{id_prefix}:auto_detect"))
                .label("Detect Roles")
                .style(ButtonStyle::Primary),
        ),
    ));

    let description_text = CreateSectionComponent::TextDisplay(CreateTextDisplay::new(
        "Use the button to automatically detect roles according to configurable patterns.
\nSelect a category to view and manually edit associated roles.",
    ));

    let description_section = CreateComponent::Section(CreateSection::new(
        vec![description_text],
        CreateSectionAccessory::Button(
            CreateButton::new(format!("{id_prefix}:edit_patterns"))
                .label("Edit Auto-detection Patterns")
                .style(ButtonStyle::Secondary),
        ),
    ));

    let category_options = vec![
        CreateSelectMenuOption::new("Bingo Rank Roles", "bingo_rank")
            .default_selection(session_state.kind == RoleMappingKindRaw::BingoRank),
        CreateSelectMenuOption::new("Blackout Roles", "completions")
            .default_selection(session_state.kind == RoleMappingKindRaw::Completions),
        CreateSelectMenuOption::new("Specific Blackout Roles", "specific_completion")
            .default_selection(session_state.kind == RoleMappingKindRaw::SpecificCompletion),
        CreateSelectMenuOption::new("Network Bingo Roles", "network_bingo")
            .default_selection(session_state.kind == RoleMappingKindRaw::NetworkBingo),
        CreateSelectMenuOption::new("Immortal Role", "immortal")
            .default_selection(session_state.kind == RoleMappingKindRaw::Immortal),
    ];

    let category_select = CreateComponent::ActionRow(CreateActionRow::SelectMenu(
        CreateSelectMenu::new(
            format!("{id_prefix}:category"),
            poise::serenity_prelude::CreateSelectMenuKind::String {
                options: category_options.into(),
            },
        )
        .placeholder("Select a category.")
        .min_values(1)
        .max_values(1),
    ));

    let create_button = CreateButton::new(format!("{id_prefix}:create_mapping"))
        .label("Create Role Binding")
        .style(ButtonStyle::Success);

    let category_section = CreateComponent::Section(CreateSection::new(
        vec![CreateSectionComponent::TextDisplay(CreateTextDisplay::new(
            format!(
                "Showing {}–{} of {} entries.",
                chunk.range.start + 1,
                chunk.range.end,
                role_mappings.len()
            ),
        ))],
        CreateSectionAccessory::Button(create_button),
    ));

    let page_nagivation = navigation::page_navigation_jump(&id_prefix, &chunk);

    let components: Vec<_> = [
        title_section,
        description_section,
        divider.clone(),
        category_select,
        category_section,
        divider.clone(),
    ]
    .into_iter()
    .chain(role_components)
    .chain([divider, page_nagivation])
    .collect();

    let container = CreateComponent::Container(CreateContainer::new(components));

    MenuMessage::new(vec![container])
}
