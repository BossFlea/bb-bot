use anyhow::{Context as _, Result, anyhow, bail};
use either::Either;
use poise::serenity_prelude::{
    AutoArchiveDuration, ButtonStyle, CacheHttp as _, ChannelId, Component, ComponentInteraction,
    ContainerComponent, Context as SerenityContext, CreateActionRow, CreateAllowedMentions,
    CreateButton, CreateComponent, CreateContainer, CreateContainerComponent,
    CreateInteractionResponse, CreateInteractionResponseFollowup, CreateInteractionResponseMessage,
    CreateMessage, CreateSeparator, CreateTextDisplay, CreateThread, Mentionable as _,
    MessageFlags, ModalInteraction, UserId,
    colours::{css::POSITIVE, css::WARNING, roles::BLUE, roles::DARK_GREY},
};
use poise::{CooldownConfig, CooldownContext};
use tracing::info;

use crate::config::CHCHEST_CHANNEL;
use crate::error::UserError;
use crate::{
    chchest::{
        modal::ChChestReport,
        types::{
            CHCHEST_COOLDOWN, Coords, ItemKind, announcement_text, item_name, mark_closed,
            thread_name,
        },
    },
    shared::{
        BotData,
        menu::timeout::{IntoCreate, disable_components},
    },
};

pub async fn handle_interaction(
    ctx: &SerenityContext,
    interaction: Either<&ComponentInteraction, &ModalInteraction>,
    action: impl Iterator<Item = &str>,
) -> Result<()> {
    match interaction {
        Either::Left(component_interaction) => {
            info!(
                "{} triggered chchest component interaction: '{}'",
                component_interaction.user.name, component_interaction.data.custom_id
            );
            component(ctx, component_interaction, action).await
        }
        Either::Right(modal_interaction) => {
            info!(
                "{} triggered chchest modal interaction: '{}'",
                modal_interaction.user.name, modal_interaction.data.custom_id
            );
            modal(ctx, modal_interaction, action).await
        }
    }
}

async fn component(
    ctx: &SerenityContext,
    interaction: &ComponentInteraction,
    mut action: impl Iterator<Item = &str>,
) -> Result<()> {
    match action.next().unwrap_or_default() {
        "close" => close(ctx, interaction, action).await,
        _ => Err(anyhow!("Invalid interaction: Unexpected action")),
    }
}

async fn modal(
    ctx: &SerenityContext,
    interaction: &ModalInteraction,
    mut action: impl Iterator<Item = &str>,
) -> Result<()> {
    match action.next().unwrap_or_default() {
        "submit" => submit(ctx, interaction).await,
        _ => Err(anyhow!("Invalid interaction: Unexpected action")),
    }
}

async fn submit(ctx: &SerenityContext, interaction: &ModalInteraction) -> Result<()> {
    let values = ChChestReport::validate(&interaction.data.components)?;

    if values.kind.is_empty() {
        bail!("Invalid modal data: Expected selected option");
    }
    let kinds = values
        .kind
        .iter()
        .map(|slug| ItemKind::from_slug(slug))
        .collect::<Result<Vec<_>>>()?;

    let custom_raw = values.custom_item.to_string();
    let custom_item = (!custom_raw.trim().is_empty()).then(|| custom_raw.trim().to_string());

    let has_custom = kinds.iter().any(|kind| kind.is_custom());
    if has_custom && custom_item.is_none() {
        bail!(UserError(anyhow!(
            "`Item found` field is required when reporting a custom CH Chest"
        )));
    }
    if !has_custom && custom_item.is_some() {
        bail!(UserError(anyhow!(
            "You entered a custom item without selecting `CH Chest`: \
            either select it or clear the `Item found` field"
        )));
    }

    let coords_list = Coords::parse_list(values.coords.as_ref())?;

    let contact_raw = values.contact.to_string();
    let contact = (!contact_raw.trim().is_empty()).then(|| contact_raw.trim().to_string());

    let notes_raw = values.notes.to_string();
    let notes = (!notes_raw.trim().is_empty()).then(|| notes_raw.trim().to_string());

    {
        let cooldown_ctx = CooldownContext {
            user_id: interaction.user.id,
            guild_id: interaction.guild_id,
            channel_id: interaction.channel_id,
        };
        let config = CooldownConfig {
            user: Some(CHCHEST_COOLDOWN),
            ..Default::default()
        };
        let data = ctx.data::<BotData>();
        let mut cooldowns = data.chchest_cooldowns.lock().await;
        if let Some(remaining) = cooldowns.remaining_cooldown(cooldown_ctx.clone(), &config) {
            let container = CreateComponent::Container(
                CreateContainer::new(vec![CreateContainerComponent::TextDisplay(
                    CreateTextDisplay::new(format!(
                        "## Slow down
You're on cooldown. Please wait {} seconds before reporting again.",
                        remaining.as_secs()
                    )),
                )])
                .accent_color(WARNING),
            );
            interaction
                .create_response(
                    ctx.http(),
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .flags(MessageFlags::IS_COMPONENTS_V2)
                            .components(vec![container])
                            .ephemeral(true),
                    ),
                )
                .await?;
            return Ok(());
        }
        cooldowns.start_cooldown(cooldown_ctx);
    }

    interaction.defer_ephemeral(ctx.http()).await?;

    let reporter = interaction.user.id;
    let mut thread_item = kinds
        .first()
        .map(|kind| item_name(*kind, custom_item.as_deref()))
        .unwrap_or_default();
    if kinds.len() > 1 {
        thread_item.push_str(&format!(" +{}", kinds.len() - 1));
    }
    let body = announcement_text(
        &kinds,
        custom_item.as_deref(),
        &coords_list,
        reporter,
        contact.as_deref(),
        notes.as_deref(),
    );

    let ping_roles = ItemKind::ping_roles(&kinds);
    let footnote = format!(
        "Find your own item? Run `/chchest` in {} to send it here!\n{}",
        ChannelId::new(916556586980347904).mention(),
        ping_roles
            .iter()
            .map(|role| role.mention().to_string())
            .collect::<Vec<_>>()
            .join(" "),
    );

    let container = CreateComponent::Container(
        CreateContainer::new(vec![
            CreateContainerComponent::TextDisplay(CreateTextDisplay::new(body)),
            CreateContainerComponent::Separator(CreateSeparator::new(true)),
            CreateContainerComponent::TextDisplay(CreateTextDisplay::new(footnote)),
            CreateContainerComponent::ActionRow(CreateActionRow::Buttons(
                vec![close_button(reporter)].into(),
            )),
        ])
        .accent_color(BLUE),
    );

    let message = CHCHEST_CHANNEL
        .send_message(
            ctx.http(),
            CreateMessage::new()
                .flags(MessageFlags::IS_COMPONENTS_V2)
                .allowed_mentions(
                    CreateAllowedMentions::new()
                        .roles(ping_roles.as_slice())
                        .users(&[reporter]),
                )
                .components(vec![container]),
        )
        .await?;

    if let Err(err) = ChannelId::new(CHCHEST_CHANNEL.get())
        .create_thread_from_message(
            ctx.http(),
            message.id,
            CreateThread::new(thread_name(&thread_item, &interaction.user.name))
                .auto_archive_duration(AutoArchiveDuration::OneDay),
        )
        .await
    {
        return Err(err).context(anyhow!("Unable to create thread for CH Chest post"));
    };

    let reply = CreateComponent::Container(
        CreateContainer::new(vec![CreateContainerComponent::TextDisplay(
            CreateTextDisplay::new(format!(
                "## Reported Successfully
Your report was posted in {}.
Use the button attached to the message to mark it as closed when it's no longer available.",
                CHCHEST_CHANNEL.mention()
            )),
        )])
        .accent_color(POSITIVE),
    );

    interaction
        .create_followup(
            ctx.http(),
            CreateInteractionResponseFollowup::new()
                .flags(MessageFlags::IS_COMPONENTS_V2)
                .components(vec![reply])
                .ephemeral(true),
        )
        .await?;

    Ok(())
}

fn close_button(reporter: UserId) -> CreateButton<'static> {
    CreateButton::new(format!("chchest:close:{}", reporter.get()))
        .label("Mark as Closed")
        .style(ButtonStyle::Danger)
}

async fn close(
    ctx: &SerenityContext,
    interaction: &ComponentInteraction,
    mut action: impl Iterator<Item = &str>,
) -> Result<()> {
    let reporter_id = interaction.user.id;
    let expected_reporter = action
        .next()
        .unwrap_or_default()
        .parse::<u64>()
        .context("Invalid interaction: Expected reporter ID")
        .map(UserId::new)?;

    let is_staff = interaction
        .member
        .as_ref()
        .and_then(|member| member.permissions)
        .is_some_and(|permissions| permissions.manage_messages());

    if reporter_id != expected_reporter && !is_staff {
        bail!(UserError(anyhow!(
            "Only the reporter or staff can mark this lobby as closed",
        )));
    }

    let mut old_components = interaction.message.components.clone();

    disable_components(&mut old_components);

    let old_text = old_components
        .iter_mut()
        .find_map(|component| match component {
            Component::Container(container) => {
                container
                    .components
                    .iter_mut()
                    .find_map(|inner| match inner {
                        ContainerComponent::TextDisplay(text) => text.content.as_mut(),
                        _ => None,
                    })
            }
            _ => None,
        })
        .context("Failed to read announcement content")?;

    *old_text = mark_closed(old_text)?;

    let mut components = old_components
        .into_iter()
        .map(IntoCreate::into_create)
        .collect::<Vec<_>>();

    for component in &mut components {
        if let CreateComponent::Container(container) = component {
            *container = std::mem::replace(container, CreateContainer::new(Vec::new()))
                .accent_color(DARK_GREY);
        }
    }

    interaction
        .create_response(
            ctx.http(),
            CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::new()
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(components),
            ),
        )
        .await?;

    Ok(())
}
