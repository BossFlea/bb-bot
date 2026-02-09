use poise::serenity_prelude::{
    ButtonStyle, CreateButton, CreateContainerComponent, CreateSection, CreateSectionAccessory,
    CreateSectionComponent, CreateTextDisplay,
};

use crate::shared::types::Bingo;

#[derive(Debug, Clone)]
pub enum HobEntry {
    OneOff {
        id: u64,
        title: String,
        comment: Option<String>,
        bingo: Bingo,
        players: OneOffPlayers,
    },
    Ongoing {
        id: u64,
        title: String,
        comment: Option<String>,
        subentries: Vec<OngoingSubentry>,
    },
}

impl HobEntry {
    pub fn get_bingo_num(&self) -> u8 {
        match self {
            HobEntry::OneOff { bingo, .. } => bingo.get_id(),
            HobEntry::Ongoing { subentries, .. } => {
                if let Some(subentry) = subentries.first() {
                    subentry.bingo.get_id()
                } else {
                    0
                }
            }
        }
    }

    pub fn to_section_edit(&self, id_prefix: &str) -> CreateContainerComponent<'static> {
        match self {
            HobEntry::OneOff {
                id,
                title,
                comment: _,
                bingo,
                players,
            } => {
                let edit_button = CreateSectionAccessory::Button(
                    CreateButton::new(format!("{id_prefix}:view_entry:{id}"))
                        .emoji('📝')
                        .style(ButtonStyle::Primary),
                );
                let text = CreateSectionComponent::TextDisplay(CreateTextDisplay::new(format!(
                    "###  {title}\n{} during {bingo}",
                    players.to_list()
                )));

                CreateContainerComponent::Section(CreateSection::new(vec![text], edit_button))
            }
            HobEntry::Ongoing {
                id,
                title,
                comment: _,
                subentries,
            } => {
                let edit_button = CreateSectionAccessory::Button(
                    CreateButton::new(format!("{id_prefix}:view_entry:{id}"))
                        .emoji('📝')
                        .style(ButtonStyle::Primary),
                );
                let description = match subentries.first() {
                    Some(subentry) => {
                        format!(
                            "`{}` and {} others\nmost recently during {}",
                            subentry.player,
                            subentries.len() - 1,
                            subentry.bingo
                        )
                    }
                    None => "*No Players*".to_string(),
                };
                let text = CreateSectionComponent::TextDisplay(CreateTextDisplay::new(format!(
                    "###  {title}\n{description}"
                )));

                CreateContainerComponent::Section(CreateSection::new(vec![text], edit_button))
            }
        }
    }

    pub fn to_text_display(&self) -> (CreateContainerComponent<'static>, usize) {
        match self {
            HobEntry::OneOff {
                title,
                comment,
                bingo,
                players,
                ..
            } => {
                let comment = comment
                    .as_ref()
                    .and_then(|c| (!c.is_empty()).then_some(format!("-# {c}")))
                    .unwrap_or_default();
                let text = format!(
                    "###  {title}\n{} during {bingo}\n{comment}",
                    players.to_list()
                );
                let length = text.len();
                let component = CreateContainerComponent::TextDisplay(CreateTextDisplay::new(text));
                (component, length)
            }
            HobEntry::Ongoing {
                title,
                comment,
                subentries,
                ..
            } => {
                let comment = comment
                    .as_ref()
                    .and_then(|c| (!c.is_empty()).then_some(format!("-# {c}")))
                    .unwrap_or_default();
                let list = if subentries.is_empty() {
                    "*No Players*".to_string()
                } else {
                    subentries
                        .iter()
                        .map(OngoingSubentry::to_list_item)
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                let text = format!("###  {title}\n{list}\n{comment}");
                let length = text.len();
                let component = CreateContainerComponent::TextDisplay(CreateTextDisplay::new(text));
                (component, length)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct OngoingSubentry {
    pub id: u64,
    pub entry_id: u64,
    pub player: String,
    pub value: String,
    pub bingo: Bingo,
}

impl OngoingSubentry {
    pub fn to_list_item(&self) -> String {
        format!(
            "- `{}` – **{}** during {}",
            self.player, self.value, self.bingo
        )
    }

    pub fn to_section(&self, id_prefix: &str) -> CreateContainerComponent<'static> {
        let text = CreateSectionComponent::TextDisplay(CreateTextDisplay::new(format!(
            "### `{}` – {}\nduring {}",
            self.player, self.value, self.bingo
        )));
        let button = CreateSectionAccessory::Button(
            CreateButton::new(format!("{id_prefix}:view_subentry:{}", self.id))
                .emoji('📝')
                .style(ButtonStyle::Primary),
        );

        CreateContainerComponent::Section(CreateSection::new(vec![text], button))
    }
}

#[derive(Debug, Clone)]
pub struct OneOffPlayers {
    pub players: Vec<String>,
}

impl OneOffPlayers {
    fn format_list<F>(&self, mut fmt: F) -> String
    where
        F: FnMut(&str) -> String,
    {
        match self.players.len() {
            0 => String::new(),
            1 => fmt(&self.players[0]).to_string(),
            _ => {
                let all_but_last = &self.players[0..self.players.len() - 1]
                    .iter()
                    .map(|player| fmt(player))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{} & {}",
                    all_but_last,
                    fmt(&self.players[self.players.len() - 1]),
                )
            }
        }
    }

    pub fn to_list(&self) -> String {
        self.format_list(|player| format!("`{player}`"))
    }

    pub fn to_plain_list(&self) -> String {
        self.format_list(str::to_string)
    }
}
