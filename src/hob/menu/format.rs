use anyhow::{Result, bail};
use poise::serenity_prelude::{
    CreateComponent, CreateContainer, CreateSeparator, CreateTextDisplay, Timestamp,
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
        CreateComponent::TextDisplay(CreateTextDisplay::new(footer)),
        footer_length,
    ));

    let mut containers: Vec<CreateContainer> = Vec::new();
    let mut current_container: Vec<CreateComponent> = vec![CreateComponent::TextDisplay(
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
            current_container.push(CreateComponent::Separator(CreateSeparator::new(true)));
        }
        current_container.push(text_display);
    }

    containers.push(CreateContainer::new(current_container).accent_color(ACCENT_COLOR));
    Ok(containers
        .into_iter()
        .map(CreateComponent::Container)
        .collect())
}
