use poise::serenity_prelude::InputTextStyle;

use bb_bot_macros::define_modal;

use crate::shared::interaction::modal::BINGO_SYNTAX;

define_modal! {
    HobEntryOneoff {
        custom_id: "oneoff_submit",
        title: "One-off Entry",
        components: [
            input title {
                style: InputTextStyle::Short,
                label: "Title",
                placeholder: "Describe the accomplishment",
                max_length: 200,
                required: true,
            },
            input players {
                style: InputTextStyle::Short,
                label: "Players",
                description: "Note: Preserves order",
                placeholder: "Enter the usernames, separated by commas",
                max_length: 200,
                required: true,
            },
            input bingo {
                style: InputTextStyle::Short,
                label: "Bingo",
                placeholder: "Enter the bingo identifier (scroll for syntax)",
                max_length: 20,
                required: true,
            },
            input comment {
                style: InputTextStyle::Short,
                label: "Comment (optional)",
                description: "Will be shown below the entry",
                placeholder: "Enter an additional comment",
                max_length: 500,
                required: false,
            },
            text {
                content: BINGO_SYNTAX,
            },
        ]
    }
}

define_modal! {
    HobEntryOngoing {
        custom_id: "ongoing_submit",
        title: "Iterative Entry",
        components: [
            input title {
                style: InputTextStyle::Short,
                label: "Title",
                placeholder: "Describe the accomplishment",
                max_length: 200,
                required: true,
            },
            input comment {
                style: InputTextStyle::Short,
                label: "Comment (optional)",
                description: "Will be shown below the entry",
                placeholder: "Enter an additional comment",
                max_length: 500,
                required: false,
            },
        ]
    }
}

define_modal! {
    HobOngoingSubentry {
        custom_id: "subentry_submit",
        title: "Subentry",
        components: [
            input player {
                style: InputTextStyle::Short,
                label: "Player",
                placeholder: "Enter the player's username",
                max_length: 20,
                required: true,
            },
            input value {
                style: InputTextStyle::Short,
                label: "Value",
                description: "Note: This can be any string (subentry sorting is based on bingo ID)",
                placeholder: "Enter the achieved value/score",
                max_length: 50,
                required: true,
            },
            input bingo {
                style: InputTextStyle::Short,
                label: "Bingo",
                placeholder: "Enter the bingo identifier (scroll for syntax)",
                max_length: 20,
                required: true,
            },
            text {
                content: BINGO_SYNTAX,
            },
        ]
    }
}
