use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Result;
use poise::serenity_prelude::{
    async_trait, small_fixed_array::FixedArray, ActionRow, ActionRowComponent, Button, ButtonKind,
    Component, ComponentType, Container, CreateActionRow, CreateButton, CreateComponent,
    CreateContainer, CreateFile, CreateInputText, CreateMediaGallery, CreateMediaGalleryItem,
    CreateSection, CreateSectionAccessory, CreateSectionComponent, CreateSelectMenu,
    CreateSelectMenuKind, CreateSelectMenuOption, CreateSeparator, CreateTextDisplay,
    CreateThumbnail, CreateUnfurledMediaItem, FileComponent, GenericChannelId, Http, InputText,
    MediaGallery, MediaGalleryItem, MessageId, Section, SelectMenu, SelectMenuOption, Separator,
    SeparatorSpacingSize, Spacing, TextDisplay, Thumbnail, UnfurledMediaItem,
};
use tokio::{
    select,
    sync::{Mutex, Notify},
};
use tracing::{error, info};

#[async_trait]
pub trait Expirable: Send + Sync + 'static {
    fn message_ids(&self) -> (&GenericChannelId, &MessageId);

    async fn invalidate<'a>(&'a self, http: Arc<Http>) -> Result<&'a str>;

    fn disable_components(components: &mut FixedArray<Component>) {
        components.iter_mut().for_each(|c| match c {
            Component::ActionRow(action_row) => {
                for component in &mut action_row.components {
                    match component {
                        ActionRowComponent::Button(button) => button.disabled = true,
                        ActionRowComponent::SelectMenu(select_menu) => select_menu.disabled = true,
                        _ => (),
                    }
                }
            }
            Component::Button(button) => button.disabled = true,
            Component::SelectMenu(select_menu) => select_menu.disabled = true,
            Component::Section(section) => {
                if let Component::Button(button) = &mut *section.accessory {
                    button.disabled = true
                }
            }
            Component::Container(container) => Self::disable_components(&mut container.components),
            _ => (),
        });
    }
}

pub async fn spawn_timeout<T>(
    http: Arc<Http>,
    sessions: Arc<Mutex<HashMap<u64, Arc<Mutex<T>>>>>,
    session_id: u64,
    timeout: Duration,
    reset_rx: Arc<Notify>,
) where
    T: Expirable,
{
    tokio::spawn(async move {
        loop {
            select! {
                _ = tokio::time::sleep(timeout) => break,
                _ = reset_rx.notified() => continue,
            };
        }

        if let Some(menu_mutex) = sessions.lock().await.remove(&session_id) {
            let menu = menu_mutex.lock().await;

            match menu.invalidate(http).await {
                Ok(name) => info!("Successfully invalidated {}'s menu message", name),
                Err(err) => {
                    error!("Unable to invalidate menu message: {err:#}")
                }
            }
        }
    });
}

pub trait IntoCreate {
    type Builder;

    fn into_create(self) -> Self::Builder;
}

impl IntoCreate for Component {
    type Builder = CreateComponent<'static>;

    fn into_create(self) -> Self::Builder {
        match self {
            Component::ActionRow(action_row) => {
                CreateComponent::ActionRow(action_row.into_create())
            }
            Component::Section(section) => CreateComponent::Section(section.into_create()),
            Component::TextDisplay(text_display) => {
                CreateComponent::TextDisplay(text_display.into_create())
            }
            Component::MediaGallery(media_gallery) => {
                CreateComponent::MediaGallery(media_gallery.into_create())
            }
            Component::Separator(separator) => CreateComponent::Separator(separator.into_create()),
            Component::File(file_component) => CreateComponent::File(file_component.into_create()),
            Component::Container(container) => CreateComponent::Container(container.into_create()),
            Component::Unknown(id) => panic!("Unknown component with ID: {id}"),
            _ => unreachable!("Invalid top-level component"),
        }
    }
}

impl IntoCreate for ActionRow {
    type Builder = CreateActionRow<'static>;

    fn into_create(self) -> Self::Builder {
        let mut iter = self.components.into_iter();
        match iter.next() {
            Some(component) => match component {
                ActionRowComponent::Button(button) => {
                    let mut buttons = vec![button.into_create()];
                    for component in iter {
                        if let ActionRowComponent::Button(button) = component {
                            buttons.push(button.into_create())
                        } else {
                            unreachable!("ActionRow should not contain mixed components")
                        }
                    }
                    CreateActionRow::Buttons(buttons.into())
                }
                ActionRowComponent::SelectMenu(select_menu) => {
                    CreateActionRow::SelectMenu(select_menu.into_create())
                }
                ActionRowComponent::InputText(input_text) => {
                    CreateActionRow::InputText(input_text.into_create())
                }
                _ => panic!("Unknown component"),
            },
            None => unreachable!("ActionRow should always contain components"),
        }
    }
}

impl IntoCreate for Button {
    type Builder = CreateButton<'static>;

    fn into_create(self) -> Self::Builder {
        let mut builder = match self.data {
            ButtonKind::Link { url } => CreateButton::new_link(url),
            ButtonKind::Premium { sku_id } => CreateButton::new_premium(sku_id),
            ButtonKind::NonLink { custom_id, style } => CreateButton::new(custom_id).style(style),
        };
        if let Some(label) = self.label {
            builder = builder.label(label)
        }
        if let Some(emoji) = self.emoji {
            builder = builder.emoji(emoji)
        }
        builder.disabled(self.disabled)
    }
}

impl IntoCreate for SelectMenu {
    type Builder = CreateSelectMenu<'static>;

    fn into_create(self) -> Self::Builder {
        let kind = match self.kind {
            ComponentType::StringSelect => CreateSelectMenuKind::String {
                options: self
                    .options
                    .into_iter()
                    .map(SelectMenuOption::into_create)
                    .collect(),
            },
            ComponentType::UserSelect => CreateSelectMenuKind::User {
                default_users: None,
            },
            ComponentType::RoleSelect => CreateSelectMenuKind::Role {
                default_roles: None,
            },
            ComponentType::MentionableSelect => CreateSelectMenuKind::Mentionable {
                default_users: None,
                default_roles: None,
            },
            ComponentType::ChannelSelect => CreateSelectMenuKind::Channel {
                channel_types: Some(self.channel_types.into()),
                default_channels: None,
            },
            _ => unreachable!("SelectMenu should always be a SelectMenu"),
        };

        let mut builder = CreateSelectMenu::new(self.custom_id, kind);
        if let Some(placeholder) = self.placeholder {
            builder = builder.placeholder(placeholder)
        }
        if let Some(min_values) = self.min_values {
            builder = builder.min_values(min_values)
        }
        if let Some(max_values) = self.max_values {
            builder = builder.max_values(max_values)
        }
        builder = builder.disabled(self.disabled);
        builder
    }
}

impl IntoCreate for SelectMenuOption {
    type Builder = CreateSelectMenuOption<'static>;

    fn into_create(self) -> Self::Builder {
        let mut builder = CreateSelectMenuOption::new(self.label, self.value);

        if let Some(description) = self.description {
            builder = builder.description(description)
        }
        if let Some(emoji) = self.emoji {
            builder = builder.emoji(emoji)
        }
        builder.default_selection(self.default)
    }
}

impl IntoCreate for InputText {
    type Builder = CreateInputText<'static>;

    fn into_create(self) -> Self::Builder {
        let style = self
            .style
            .expect("style should always be present on InputText");
        let label = self
            .label
            .expect("label should always be present on InputText");
        let mut builder = CreateInputText::new(style, self.custom_id).label(label);
        if let Some(min_length) = self.min_length {
            builder = builder.min_length(min_length)
        }
        if let Some(max_length) = self.max_length {
            builder = builder.max_length(max_length)
        }
        if let Some(value) = self.value {
            builder = builder.value(value)
        }
        if let Some(placeholder) = self.placeholder {
            builder = builder.placeholder(placeholder)
        }
        builder.required(self.required)
    }
}

impl IntoCreate for Section {
    type Builder = CreateSection<'static>;

    fn into_create(self) -> Self::Builder {
        let components: Vec<_> = self
            .components
            .into_iter()
            .map(|c| match c {
                Component::TextDisplay(text_display) => {
                    CreateSectionComponent::TextDisplay(text_display.into_create())
                }
                _ => unreachable!("Invalid Section sub-component"),
            })
            .collect();
        let accessory = match *self.accessory {
            Component::Button(button) => CreateSectionAccessory::Button(button.into_create()),
            Component::Thumbnail(thumbnail) => {
                CreateSectionAccessory::Thumbnail(thumbnail.into_create())
            }
            _ => unreachable!("Invalid Section accessory"),
        };
        CreateSection::new(components, accessory)
    }
}

impl IntoCreate for TextDisplay {
    type Builder = CreateTextDisplay<'static>;

    fn into_create(self) -> Self::Builder {
        CreateTextDisplay::new(self.content.unwrap_or_default())
    }
}

impl IntoCreate for Thumbnail {
    type Builder = CreateThumbnail<'static>;

    fn into_create(self) -> Self::Builder {
        let mut builder = CreateThumbnail::new(self.media.into_create());

        if let Some(description) = self.description {
            builder = builder.description(description)
        }
        if let Some(spoiler) = self.spoiler {
            builder = builder.spoiler(spoiler)
        }
        builder
    }
}

impl IntoCreate for UnfurledMediaItem {
    type Builder = CreateUnfurledMediaItem<'static>;

    fn into_create(self) -> Self::Builder {
        CreateUnfurledMediaItem::new(self.url)
    }
}

impl IntoCreate for MediaGallery {
    type Builder = CreateMediaGallery<'static>;

    fn into_create(self) -> Self::Builder {
        let items: Vec<_> = self
            .items
            .into_iter()
            .map(MediaGalleryItem::into_create)
            .collect();
        CreateMediaGallery::new(items)
    }
}

impl IntoCreate for MediaGalleryItem {
    type Builder = CreateMediaGalleryItem<'static>;

    fn into_create(self) -> Self::Builder {
        let mut builder = CreateMediaGalleryItem::new(self.media.into_create());

        if let Some(description) = self.description {
            builder = builder.description(description)
        }
        if let Some(spoiler) = self.spoiler {
            builder = builder.spoiler(spoiler)
        }
        builder
    }
}

impl IntoCreate for Separator {
    type Builder = CreateSeparator;

    fn into_create(self) -> Self::Builder {
        let divider = self
            .divider
            .expect("divider should always be present on Separator");
        let mut builder = CreateSeparator::new(divider);
        if let Some(spacing) = self.spacing {
            let spacing = match spacing {
                SeparatorSpacingSize::Small => Spacing::Small,
                SeparatorSpacingSize::Large => Spacing::Large,
                _ => panic!("Unknown Separator spacing size"),
            };
            builder = builder.spacing(spacing)
        }
        builder
    }
}

impl IntoCreate for FileComponent {
    type Builder = CreateFile<'static>;

    fn into_create(self) -> Self::Builder {
        let mut builder = CreateFile::new(self.file.into_create());
        if let Some(spoiler) = self.spoiler {
            builder = builder.spoiler(spoiler)
        }
        builder
    }
}

impl IntoCreate for Container {
    type Builder = CreateContainer<'static>;

    fn into_create(self) -> Self::Builder {
        let components: Vec<_> = self
            .components
            .into_iter()
            .map(Component::into_create)
            .collect();

        let mut builder = CreateContainer::new(components);
        if let Some(accent_color) = self.accent_color {
            builder = builder.accent_color(accent_color)
        }
        if let Some(spoiler) = self.spoiler {
            builder = builder.spoiler(spoiler)
        }
        builder
    }
}
