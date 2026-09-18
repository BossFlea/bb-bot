use poise::serenity_prelude::{CreateSelectMenuKind, CreateSelectMenuOption, InputTextStyle};

use bb_bot_macros::define_modal;

define_modal! {
    ChChestReport {
        custom_id: "submit",
        title: "Report CH Chest",
        components: [
            select kind {
                kind: CreateSelectMenuKind::String {
                    options: vec![
                        CreateSelectMenuOption::new("Electron Transmitter", "electron_transmitter"),
                        CreateSelectMenuOption::new("FTX 3070", "ftx_3070"),
                        CreateSelectMenuOption::new("Robotron Reflector", "robotron_reflector"),
                        CreateSelectMenuOption::new("Superlite Motor", "superlite_motor"),
                        CreateSelectMenuOption::new("Control Switch", "control_switch"),
                        CreateSelectMenuOption::new("Synthetic Heart", "synthetic_heart"),
                        CreateSelectMenuOption::new("Key Guardian", "key_guardian"),
                        CreateSelectMenuOption::new("CH Chest (custom loot)", "custom").description("Enter item(s) in the field below"),
                    ]
                    .into(),
                },
                label: "Chest content",
                max_values: 1,
                required: true,
            },
            input custom_item {
                style: InputTextStyle::Short,
                label: "Item(s) found (custom only)",
                description: "Required when reporting a custom CH Chest",
                placeholder: "List the relevant chest loot",
                max_length: 100,
                required: false,
            },
            input coords {
                style: InputTextStyle::Short,
                label: "Coordinates",
                description: "Accepts most common formats",
                placeholder: "e.g. 123, 64, -456",
                max_length: 50,
                required: true,
            },
            input contact {
                style: InputTextStyle::Short,
                label: "Contact (optional)",
                description: "Defaults to sending username in thread",
                placeholder: "e.g. /p <username>, /boop, send username in thread, ...",
                max_length: 100,
                required: false,
            },
            input notes {
                style: InputTextStyle::Paragraph,
                label: "Notes (optional)",
                placeholder: "Server ID, lobby day count, ...",
                max_length: 200,
                required: false,
            },
        ]
    }
}
