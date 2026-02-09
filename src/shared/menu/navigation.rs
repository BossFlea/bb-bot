use std::{cmp, ops::Range};

use anyhow::Result;
use poise::serenity_prelude::{
    ButtonStyle, CreateActionRow, CreateButton, CreateContainerComponent, ReactionType,
    async_trait, small_fixed_array::FixedString,
};

use crate::{db::DbHandle, shared::menu::MenuMessage};

pub trait BacktrackState {
    type WrapperEnum;
    fn set_referrer(&mut self, referrer_state: Self::WrapperEnum);
    fn take_referrer(&mut self) -> Option<Box<Self::WrapperEnum>>;
    fn take_referrer_or(
        &mut self,
        default: impl FnOnce() -> Self::WrapperEnum,
    ) -> Self::WrapperEnum {
        self.take_referrer().map(|r| *r).unwrap_or_else(default)
    }
}

pub trait Backtrack {
    #[allow(dead_code)] // currently unused
    fn restore_referrer_or(&mut self, default: impl FnOnce() -> Self);
    fn update_state(&mut self, new_state: Self);
}

#[async_trait]
pub trait GenerateMenu {
    async fn generate(&mut self, db: &DbHandle, menu_id: u64) -> Result<MenuMessage<'static>>;
}

pub fn page_navigation_jump(
    id_prefix: &str,
    page_chunk: &PaginatedChunk,
) -> CreateContainerComponent<'static> {
    let mut buttons = navigation_buttons_basic(id_prefix, page_chunk);

    let first_button = first_button(id_prefix, page_chunk.page == 0);
    buttons.insert(0, first_button);

    let jump_button = CreateButton::new(format!("{}:jump_page", id_prefix))
        .label("Jump to Page")
        .emoji(ReactionType::Unicode(FixedString::from_str_trunc("🔢")))
        .style(ButtonStyle::Secondary)
        .disabled(page_chunk.total_pages == 0);

    buttons.push(jump_button);

    CreateContainerComponent::ActionRow(CreateActionRow::Buttons(buttons.into()))
}

pub fn page_navigation_subentry(
    id_prefix: &str,
    page_chunk: &PaginatedChunk,
) -> CreateContainerComponent<'static> {
    let mut buttons = navigation_buttons_basic(id_prefix, page_chunk);

    let back_button = back_button(id_prefix);
    buttons.insert(0, back_button);

    CreateContainerComponent::ActionRow(CreateActionRow::Buttons(buttons.into()))
}

pub fn nagivation_back(id_prefix: &str) -> CreateContainerComponent<'static> {
    let back_button = back_button(id_prefix);

    CreateContainerComponent::ActionRow(CreateActionRow::Buttons(vec![back_button].into()))
}

fn back_button(id_prefix: &str) -> CreateButton<'static> {
    CreateButton::new(format!("{id_prefix}:back"))
        .emoji(ReactionType::Unicode(FixedString::from_str_trunc("↩️")))
        .label("Back")
        .style(ButtonStyle::Secondary)
}

fn first_button(id_prefix: &str, disabled: bool) -> CreateButton<'static> {
    CreateButton::new(format!("{}:goto_page:first", id_prefix))
        .label("First")
        .emoji(ReactionType::Unicode(FixedString::from_str_trunc("⏮️")))
        .style(ButtonStyle::Primary)
        .disabled(disabled)
}

fn navigation_buttons_basic(
    id_prefix: &str,
    page_chunk: &PaginatedChunk,
) -> Vec<CreateButton<'static>> {
    let prev_button = CreateButton::new(format!("{}:goto_page:prev", id_prefix))
        .emoji(ReactionType::Unicode(FixedString::from_str_trunc("◀️")))
        .style(ButtonStyle::Primary)
        .disabled(page_chunk.page == 0);

    let next_button = CreateButton::new(format!("{}:goto_page:next", id_prefix))
        .emoji(ReactionType::Unicode(FixedString::from_str_trunc("▶️")))
        .style(ButtonStyle::Primary)
        .disabled(page_chunk.page == page_chunk.total_pages.saturating_sub(1));

    let page_indicator = CreateButton::new("page_indicator")
        .label(format!(
            "Page {} / {}",
            page_chunk.page + 1,
            page_chunk.total_pages
        ))
        .style(ButtonStyle::Success)
        .disabled(true);

    vec![prev_button, page_indicator, next_button]
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaginatedChunk {
    pub range: Range<usize>,
    pub page: usize,
    pub total_pages: usize,
}

impl PaginatedChunk {
    /// # Panics
    /// Panics if page_size is 0
    pub fn new(length: usize, page: usize, page_size: usize) -> Self {
        assert!(page_size > 0, "page_size must be greater than 0");

        let total_pages = length.div_ceil(page_size);

        let clamped_page = if total_pages == 0 {
            0
        } else {
            cmp::min(page, total_pages - 1)
        };

        let start_index = clamped_page * page_size;
        let end_index = cmp::min(start_index + page_size, length);

        Self {
            range: start_index..end_index,
            page: start_index / page_size,
            total_pages,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paginate_build() {
        assert_eq!(
            PaginatedChunk::new(0, 0, 5),
            PaginatedChunk {
                range: 0..0,
                page: 0,
                total_pages: 0
            }
        );
        assert_eq!(
            PaginatedChunk::new(8, 0, 5),
            PaginatedChunk {
                range: 0..5,
                page: 0,
                total_pages: 1
            }
        );
        assert_eq!(
            PaginatedChunk::new(25, 1, 25),
            PaginatedChunk {
                range: 0..25,
                page: 0,
                total_pages: 1
            }
        );
        assert_eq!(
            PaginatedChunk::new(13, 2, 5),
            PaginatedChunk {
                range: 10..13,
                page: 2,
                total_pages: 3
            }
        );
        assert_eq!(
            PaginatedChunk::new(7, 3, 5),
            PaginatedChunk {
                range: 5..7,
                page: 1,
                total_pages: 2
            }
        );
    }

    #[test]
    #[should_panic]
    fn paginate_build_page_size_zero() {
        PaginatedChunk::new(15, 1, 0);
    }
}
