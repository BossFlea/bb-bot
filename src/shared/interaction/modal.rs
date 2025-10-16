use poise::serenity_prelude::InputTextStyle;

use bb_bot_macros::define_modal;

define_modal! {
    Search {
        custom_id: "search_submit",
        title: "Search (Content-inclusive)",
        components: [
            input query {
                style: InputTextStyle::Short,
                label: "Search Query",
                placeholder: "Enter a search query",
                max_length: 100,
                required: true,
            },
            text {
                content: "### Supported (SQL) wildcards
                    - `%` – anything, zero or more characters
                    - `_` – any single character
                    - `\\` – escape character (e.g. `\\%` matches a literal %)",
            },
            text {
                content: "**Note**: The search query is matched against all fields of all entries. \
                    This means the search can be used to filter by player, for example.",
            },
        ]
    }
}

define_modal! {
    JumpPage {
        custom_id: "jump_page_submit",
        title: "Jump to Page",
        components: [
            input page {
                style: InputTextStyle::Short,
                label: "Page",
                placeholder: "Enter a page number",
                max_length: 3,
                required: true,
            },
        ]
    }
}

pub const BINGO_SYNTAX: &str = "### Bingo Syntax:
- Basic Format: `[optional type] <num>`
- Types: `extreme` / `secret` / `normal`
  - Any abbreviations: `e` / `ex` / `extr` / `sec` / ...
  - Defaults to normal
- `#` is optional (`#2` = `2`)
- `bingo` is optional (`extreme bingo #2` = `extreme #2`)
- All spaces are optional
- Case-insensitive
### Examples:
- `extreme 3`, `secret 1`, `normal 12`/`12`
- `e2`, `s1`, `n5`/`5`
- `ex#2`, `sec#1`, `nor#7`/`#7`
- `Extreme Bingo #1`, `Ex#2`, `Bingo #23`";
