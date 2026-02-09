use std::sync::Arc;

use anyhow::Result;
use poise::serenity_prelude::{
    Component, CreateComponent, CreateContainer, CreateContainerComponent, CreateTextDisplay,
    GenericChannelId, Http, MessageId, User, async_trait,
};
use tokio::sync::Notify;

use crate::db::DbHandle;
use crate::role::db::role_config::GetRoleMappingsByKind;
use crate::role::types::RoleMappingKindRaw;
use crate::shared::menu::navigation::GenerateMenu;
use crate::shared::menu::{
    MenuMessage,
    timeout::{Expirable, IntoCreate as _},
};

mod configure_roles;

#[derive(Debug)]
pub struct RoleConfigSession {
    pub menu_id: u64,
    pub state: RoleConfigState,
    pub owner: User,
    pub channel_id: GenericChannelId,
    pub message_id: MessageId,
    pub timeout_reset: Arc<Notify>,
}

#[async_trait]
impl Expirable for RoleConfigSession {
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
                .add_component(CreateContainerComponent::TextDisplay(
                    CreateTextDisplay::new("-# This menu has expired."),
                ));
        }

        let menu = MenuMessage::new(component_builders);

        http.edit_message(channel_id, message_id, &menu.into_edit(), vec![])
            .await?;

        Ok(self.owner.name.as_str())
    }

    fn message_ids(&self) -> (&GenericChannelId, &MessageId) {
        (&self.channel_id, &self.message_id)
    }
}

#[derive(Debug)]
pub struct RoleConfigState {
    pub kind: RoleMappingKindRaw,
    pub page: usize,
}
impl RoleConfigState {
    pub fn new(kind: RoleMappingKindRaw, page: usize) -> Self {
        Self { kind, page }
    }
}

#[async_trait]
impl GenerateMenu for RoleConfigState {
    async fn generate(&mut self, db: &DbHandle, menu_id: u64) -> Result<MenuMessage<'static>> {
        let role_mappings = db
            .request(GetRoleMappingsByKind { kind: self.kind })
            .await??;
        Ok(configure_roles::generate(menu_id, &role_mappings, self))
    }
}
