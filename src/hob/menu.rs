use std::sync::Arc;

use anyhow::{Context as _, Result};
use poise::serenity_prelude::{
    Component, CreateComponent, CreateContainer, CreateTextDisplay, GenericChannelId, Http,
    MessageId, User, async_trait,
};
use tokio::sync::Notify;

use crate::db::DbHandle;
use crate::shared::menu::{
    MenuMessage,
    navigation::{Backtrack, BacktrackState, GenerateMenu},
    timeout::{Expirable, IntoCreate as _},
};

pub mod format;

mod select_entry;
mod view_entry;

#[derive(Debug)]
pub struct HobEditSession {
    pub menu_id: u64,
    pub state: HobEditState,
    pub owner: User,
    pub channel_id: GenericChannelId,
    pub message_id: MessageId,
    pub timeout_reset: Arc<Notify>,
}

#[async_trait]
impl Expirable for HobEditSession {
    async fn invalidate<'a>(&'a self, http: Arc<Http>) -> Result<&'a str> {
        let (&channel_id, &message_id) = self.message_ids();

        let mut components = http.get_message(channel_id, message_id).await?.components;
        Self::disable_components(&mut components);

        let mut component_builders: Vec<_> =
            components.into_iter().map(Component::into_create).collect();

        if let Some(container) = component_builders.iter_mut().rev().find_map(|c| {
            if let CreateComponent::Container(container) = c {
                Some(container)
            } else {
                None
            }
        }) {
            *container = std::mem::replace(container, CreateContainer::new(Vec::new()))
                .add_component(CreateComponent::TextDisplay(CreateTextDisplay::new(
                    "-# This menu has expired.",
                )));
        }

        let menu = MenuMessage::new(component_builders);

        http.edit_message(channel_id, message_id, &menu.into_edit(), Vec::new())
            .await?;

        Ok(self.owner.name.as_str())
    }

    fn message_ids(&self) -> (&GenericChannelId, &MessageId) {
        (&self.channel_id, &self.message_id)
    }
}

#[derive(Debug)]
pub struct SelectEntryState {
    pub page: usize,
    pub search_query: Option<String>,
}
impl SelectEntryState {
    pub fn new(page: usize, search_query: Option<String>) -> Self {
        Self { page, search_query }
    }
}

#[derive(Debug)]
pub struct ViewEntryState {
    pub id: u64,
    pub page: usize,
    referrer_state: Option<Box<HobEditState>>,
}
impl ViewEntryState {
    pub fn new(id: u64, page: usize) -> Self {
        Self {
            id,
            page,
            referrer_state: None,
        }
    }
}
impl BacktrackState for ViewEntryState {
    type WrapperEnum = HobEditState;
    fn set_referrer(&mut self, referrer_state: Self::WrapperEnum) {
        _ = self.referrer_state.insert(Box::new(referrer_state))
    }
    fn take_referrer(&mut self) -> Option<Box<Self::WrapperEnum>> {
        self.referrer_state.take()
    }
}

#[derive(Debug)]
pub struct ViewSubentryState {
    pub id: u64,
    pub entry_id: u64,
    referrer_state: Option<Box<HobEditState>>,
}
impl ViewSubentryState {
    pub fn new(id: u64, entry_id: u64) -> Self {
        Self {
            id,
            entry_id,
            referrer_state: None,
        }
    }
}
impl BacktrackState for ViewSubentryState {
    type WrapperEnum = HobEditState;
    fn set_referrer(&mut self, referrer_state: Self::WrapperEnum) {
        _ = self.referrer_state.insert(Box::new(referrer_state))
    }
    fn take_referrer(&mut self) -> Option<Box<Self::WrapperEnum>> {
        self.referrer_state.take()
    }
}

#[derive(Debug)]
pub enum HobEditState {
    SelectEntry(SelectEntryState),
    ViewEntry(ViewEntryState),
    ViewSubentry(ViewSubentryState),
}

impl Backtrack for HobEditState {
    fn restore_referrer_or(&mut self, default: impl FnOnce() -> Self) {
        match self {
            HobEditState::ViewEntry(state) => *self = state.take_referrer_or(default),
            HobEditState::ViewSubentry(state) => *self = state.take_referrer_or(default),
            _ => (),
        }
    }
    fn update_state(&mut self, new_state: HobEditState) {
        let previous_state = std::mem::replace(self, new_state);
        match self {
            HobEditState::ViewEntry(state) => state.set_referrer(previous_state),
            HobEditState::ViewSubentry(state) => state.set_referrer(previous_state),
            _ => (),
        }
    }
}

#[async_trait]
impl GenerateMenu for HobEditState {
    async fn generate(&mut self, db: &DbHandle, menu_id: u64) -> Result<MenuMessage<'static>> {
        match self {
            HobEditState::SelectEntry(state) => state.generate(db, menu_id).await,
            HobEditState::ViewEntry(state) => state.generate(db, menu_id).await,
            HobEditState::ViewSubentry(state) => state.generate(db, menu_id).await,
        }
    }
}

#[async_trait]
impl GenerateMenu for SelectEntryState {
    async fn generate(&mut self, db: &DbHandle, menu_id: u64) -> Result<MenuMessage<'static>> {
        let hob_entries = match &self.search_query {
            Some(query) => db.search_entries_with_content(query.to_string()).await?,
            None => db.get_all_hob_entries().await?,
        };
        Ok(select_entry::generate_entry_list(
            menu_id,
            &hob_entries,
            self,
        ))
    }
}

#[async_trait]
impl GenerateMenu for ViewEntryState {
    async fn generate(&mut self, db: &DbHandle, menu_id: u64) -> Result<MenuMessage<'static>> {
        let hob_entry = db
            .get_hob_entry_by_id(self.id)
            .await?
            .context("Unable to find entry by ID")?;

        Ok(view_entry::generate_entry(
            menu_id,
            hob_entry,
            &mut self.page,
        ))
    }
}

#[async_trait]
impl GenerateMenu for ViewSubentryState {
    async fn generate(&mut self, db: &DbHandle, menu_id: u64) -> Result<MenuMessage<'static>> {
        let subentry = db
            .get_ongoing_subentry_by_id(self.id, self.entry_id)
            .await?
            .context("Unable to find subentry by ID")?;

        Ok(view_entry::generate_subentry(menu_id, subentry))
    }
}
