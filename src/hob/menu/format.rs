use std::fmt::Write as _;

use anyhow::{Result, bail};
use poise::serenity_prelude::{
    CreateComponent, CreateContainer, CreateContainerComponent, CreateSeparator, CreateTextDisplay,
    Timestamp,
};

use crate::hob::types::HobEntry;
use crate::shared::menu::ACCENT_COLOR;

const MAX_CHARS: usize = 4000;
const MAX_COMPONENTS: usize = 40;

pub fn build_hob_messages(
    hob_entries: &[HobEntry],
    max_messages: usize,
) -> Result<Vec<CreateComponent<'static>>> {
    const TITLE_TEXT: &str = "# Hall of Bingo";

    let footer = format!(
        "-# Last updated: <t:{}:f>",
        Timestamp::now().unix_timestamp()
    );
    let footer_length = footer.len();

    let mut entry_texts: Vec<_> = hob_entries
        .iter()
        .rev()
        .map(HobEntry::to_text_display)
        .collect();

    entry_texts.push((
        CreateContainerComponent::TextDisplay(CreateTextDisplay::new(footer)),
        footer_length,
    ));

    let mut containers: Vec<CreateContainer> = Vec::new();
    let mut current_container: Vec<CreateContainerComponent> =
        vec![CreateContainerComponent::TextDisplay(
            CreateTextDisplay::new(TITLE_TEXT),
        )];
    let mut current_container_chars = TITLE_TEXT.len();

    for (text_display, length) in entry_texts {
        if length > MAX_CHARS {
            bail!("Single entry exceeds message character limits by itself")
        }

        // flush container (= message) if character or component limit reached
        if current_container_chars + length > MAX_CHARS
            || current_container.len() + 2 > MAX_COMPONENTS
        {
            containers.push(CreateContainer::new(current_container).accent_color(ACCENT_COLOR));
            current_container = Vec::new();
            current_container_chars = 0;

            if containers.len() >= max_messages {
                bail!(
                    "Reached configured limit of {max_messages} messages before processing all entries"
                )
            }
        }

        if !current_container.is_empty() {
            current_container.push(CreateContainerComponent::Separator(CreateSeparator::new(
                true,
            )));
        }
        current_container.push(text_display);
    }

    containers.push(CreateContainer::new(current_container).accent_color(ACCENT_COLOR));
    Ok(containers
        .into_iter()
        .map(CreateComponent::Container)
        .collect())
}

pub fn build_hob_backup_script(hob_entries: &[HobEntry]) -> String {
    let mut oneoff_entries_values: Vec<String> = Vec::new();
    let mut oneoff_players_values: Vec<String> = Vec::new();
    let mut ongoing_entries_values: Vec<String> = Vec::new();
    let mut ongoing_subentries_values: Vec<String> = Vec::new();

    for entry in hob_entries {
        match entry {
            HobEntry::OneOff {
                id,
                title,
                comment,
                bingo,
                players,
            } => {
                oneoff_entries_values.push(format!(
                    "({id}, {}, {}, {}, {})",
                    wrap_sql_string(title),
                    comment
                        .as_ref()
                        .map(wrap_sql_string)
                        .unwrap_or("NULL".to_string()),
                    bingo.kind_specific_id,
                    bingo.kind as u8,
                ));
                for (i, player) in players.players.iter().enumerate() {
                    oneoff_players_values.push(format!("({id}, {}, {i})", wrap_sql_string(player)));
                }
            }
            HobEntry::Ongoing {
                id,
                title,
                comment,
                subentries,
            } => {
                ongoing_entries_values.push(format!(
                    "({id}, {}, {})",
                    wrap_sql_string(title),
                    comment
                        .as_ref()
                        .map(wrap_sql_string)
                        .unwrap_or("NULL".to_string()),
                ));
                for subentry in subentries {
                    ongoing_subentries_values.push(format!(
                        "({}, {id}, {}, {}, {}, {})",
                        subentry.id,
                        wrap_sql_string(&subentry.player),
                        wrap_sql_string(&subentry.value),
                        subentry.bingo.kind_specific_id,
                        subentry.bingo.kind as u8,
                    ));
                }
            }
        }
    }

    let mut output = String::new();
    append_section(
        &mut output,
        "One-off entries",
        "hob_entries_oneoff",
        "id, title, comment, bingo, bingo_kind",
        &oneoff_entries_values,
    );
    output.push_str("\n\n");
    append_section(
        &mut output,
        "One-off players",
        "hob_oneoff_players",
        "entry_id, player, position",
        &oneoff_players_values,
    );
    output.push_str("\n\n");
    append_section(
        &mut output,
        "Iterative entries",
        "hob_entries_ongoing",
        "id, title, comment",
        &ongoing_entries_values,
    );
    output.push_str("\n\n");
    append_section(
        &mut output,
        "Iterative subentries",
        "hob_ongoing_subentries",
        "id, entry_id, player, value, bingo, bingo_kind",
        &ongoing_subentries_values,
    );

    output
}

fn wrap_sql_string<T: AsRef<str>>(value: T) -> String {
    let s = value.as_ref();
    let escaped = s.replace("'", "''");
    format!("'{escaped}'")
}

fn append_section(
    output: &mut String,
    comment: &str,
    table: &str,
    columns: &str,
    values: &[String],
) {
    writeln!(output, "-- {comment}\nDELETE FROM {table};").unwrap();

    if !values.is_empty() {
        writeln!(output, "INSERT INTO {table} ({columns}) VALUES").unwrap();
        for (i, val) in values.iter().enumerate() {
            if i != 0 {
                output.push_str(",\n");
            }
            write!(output, "{val}").unwrap();
        }
        output.push(';');
    }
}
