use poise::{
    CreateReply,
    serenity_prelude::{
        Color, CreateAllowedMentions, CreateComponent, CreateInteractionResponseMessage,
        EditMessage, MessageFlags,
    },
};

pub mod navigation;
pub mod timeout;

pub const ACCENT_COLOR: Color = Color::BLUE;

#[derive(Debug, Clone)]
pub struct MenuMessage<'a> {
    pub components: Vec<CreateComponent<'a>>,
}

impl<'a> MenuMessage<'a> {
    pub fn new(components: Vec<CreateComponent<'a>>) -> Self {
        Self { components }
    }

    pub fn into_reply(self) -> CreateReply<'a> {
        CreateReply::default()
            .flags(MessageFlags::IS_COMPONENTS_V2)
            .allowed_mentions(CreateAllowedMentions::new())
            .components(self.components)
    }

    pub fn into_edit(self) -> EditMessage<'a> {
        EditMessage::default()
            .flags(MessageFlags::IS_COMPONENTS_V2)
            .allowed_mentions(CreateAllowedMentions::new())
            .components(self.components)
    }

    pub fn into_interaction_response(self) -> CreateInteractionResponseMessage<'a> {
        CreateInteractionResponseMessage::default()
            .flags(MessageFlags::IS_COMPONENTS_V2)
            .allowed_mentions(CreateAllowedMentions::new())
            .components(self.components)
    }
}

/// Generate an ID that can be stored as i64 in SQLite
pub fn generate_id() -> u64 {
    rand::random_range(0..=i64::MAX as u64)
}
