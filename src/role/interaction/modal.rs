use poise::serenity_prelude::{CreateSelectMenuKind, CreateSelectMenuOption, InputTextStyle};

use bb_bot_macros::define_modal;

use crate::role::types::NetworkBingo;

define_modal! {
    RoleRequestLink {
        custom_id: "confirm_link_submit",
        title: "Link Accounts",
        components: [
            input username {
                style: InputTextStyle::Short,
                label: "Username",
                placeholder: "Enter Minecraft username",
                max_length: 16,
                required: true,
            }
        ]
    }
}

const PATTERN_INSTRUCTIONS: &str = "## Role Patterns
These template patterns are used to automatically detect existing roles. \
Placeholders will be replaced with the corresponding values and matched against role names.
### Placeholders
- `{rank}` – Bingo Rank number
- `{count}` – Number of Blackouts
- `{kind}` – Bingo type identifier
  - Empty if type is Normal
- `{number}` – Bingo ID number (kind-specific)
### Examples
- `Bingo Rank {rank}` → `Bingo Rank 4`
- `Blackouts: {count}` → `Blackouts: 12`
- `{kind}Bingo #{number}` → `Extreme Bingo #2`";

define_modal! {
    RolePatterns {
        custom_id: "role_patterns_submit",
        title: "Configure Auto-detection Patterns",
        components: [
            text {
                content: PATTERN_INSTRUCTIONS,
            },
            input bingo_rank {
                style: InputTextStyle::Short,
                label: "Bingo Rank Roles",
                description: "Available placeholders: {rank}",
                placeholder: "Enter a pattern for Bingo Rank roles",
                max_length: 100,
                required: true,
            },
            input completions {
                style: InputTextStyle::Short,
                label: "Blackout Count Roles",
                description: "Available placeholders: {count}",
                placeholder: "Enter a pattern for Blackout roles",
                max_length: 100,
                required: true,
            },
            input specific_completion {
                style: InputTextStyle::Short,
                label: "Specific Blackout Roles",
                description: "Available placeholders: {kind}, {number}",
                placeholder: "Enter a pattern for Specific Blackout roles",
                max_length: 100,
                required: true,
            },
            input immortal {
                style: InputTextStyle::Short,
                label: "Immortal Role",
                placeholder: "Enter a pattern for the Immortal role",
                max_length: 100,
                required: true,
            },
        ]
    }
}

define_modal! {
    RoleMappingBingoRank {
        custom_id: "role_mapping_bingo_rank_submit",
        title: "New Role Binding: Bingo Rank",
        components: [
            input rank {
                style: InputTextStyle::Short,
                label: "Rank",
                description: "The Bingo Rank tier the role represents",
                placeholder: "Enter a number",
                max_length: 2,
                required: true,
            },
            input role_id {
                style: InputTextStyle::Short,
                label: "Role",
                description: "The Role ID to associate with the given Bingo Rank",
                placeholder: "Enter a role ID",
                max_length: 20,
                required: true,
            }
        ]
    }
}

define_modal! {
    RoleMappingCompletions {
        custom_id: "role_mapping_completions_submit",
        title: "New Role Binding: Blackout Count",
        components: [
            input count {
                style: InputTextStyle::Short,
                label: "Count",
                description: "The number of Blackouts the role represents",
                placeholder: "Enter a number",
                max_length: 3,
                required: true,
            },
            input role_id {
                style: InputTextStyle::Short,
                label: "Role",
                description: "The Role ID to associate with the given Blackout count",
                placeholder: "Enter a role ID",
                max_length: 20,
                required: true,
            }
        ]
    }
}

define_modal! {
    RoleMappingSpecificCompletion {
        custom_id: "role_mapping_specific_completion_submit",
        title: "New Role Binding: Specific Blackout",
        components: [
            select bingo_kind {
                kind: CreateSelectMenuKind::String {
                    options: vec![
                        CreateSelectMenuOption::new("Normal Bingo", "0").default_selection(true),
                        CreateSelectMenuOption::new("Extreme Bingo", "1"),
                        CreateSelectMenuOption::new("Secret Bingo", "2"),
                    ]
                    .into(),
                },
                label: "Bingo Kind",
                description: "The type of the Bingo the role represents",
                max_values: 1,
                required: true,
            },
            input kind_specific_id {
                style: InputTextStyle::Short,
                label: "Bingo ID (kind-specific)",
                description: "The ID of the Bingo the role represents",
                placeholder: "Enter a number",
                max_length: 3,
                required: true,
            },
            input role_id {
                style: InputTextStyle::Short,
                label: "Role",
                description: "The Role ID to associate with the given Bingo event",
                placeholder: "Enter a role ID",
                max_length: 20,
                required: true,
            }
        ]
    }
}

define_modal! {
    RoleMappingNetworkBingo {
        custom_id: "role_mapping_network_bingo_submit",
        title: "New Role Binding: Network Bingo",
        components: [
            select bingo {
                kind: CreateSelectMenuKind::String {
                    options: NetworkBingo::ALL
                        .iter()
                        .map(|b| CreateSelectMenuOption::new(b.to_string(), (*b as u8).to_string()))
                        .collect(),
                },
                label: "Bingo",
                description: "The Bingo event the role represents",
                max_values: 1,
                required: true,
            },
            input role_id {
                style: InputTextStyle::Short,
                label: "Role",
                description: "The Role ID to associate with the given Bingo event",
                placeholder: "Enter a role ID",
                max_length: 20,
                required: true,
            }
        ]
    }
}

define_modal! {
    RoleMappingImmortal {
        custom_id: "role_mapping_immortal_submit",
        title: "New Role Binding: Immortal",
        components: [
            input role_id {
                style: InputTextStyle::Short,
                label: "Role",
                description: "The Role ID to associate with the Immortal achievement",
                placeholder: "Enter a role ID",
                max_length: 20,
                required: true,
            }
        ]
    }
}
