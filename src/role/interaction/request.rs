use anyhow::{Context as _, Result, anyhow, bail};
use either::Either;
use poise::serenity_prelude::{
    ButtonStyle, CacheHttp as _, ComponentInteraction, Context as SerenityContext, CreateButton,
    CreateComponent, CreateContainer, CreateInteractionResponse, CreateInteractionResponseFollowup,
    CreateInteractionResponseMessage, CreateMediaGallery, CreateMediaGalleryItem, CreateSection,
    CreateSectionAccessory, CreateSectionComponent, CreateTextDisplay, CreateUnfurledMediaItem,
    Mentionable as _, MessageFlags, ModalInteraction,
    colours::{css::POSITIVE, roles::BLUE},
};
use tracing::info;

use crate::config::{BOT_MAINTAINER, MANUAL_ROLE_CHANNEL};
use crate::role::{
    db::link::{GetLinkedUserByDiscord, RemoveLinkedUserByDiscord, RemoveLinkedUserByMinecraft},
    interaction::modal,
};
use crate::shared::BotData;

pub async fn handle_interaction(
    ctx: &SerenityContext,
    interaction: Either<&ComponentInteraction, &ModalInteraction>,
    action: impl Iterator<Item = &str>,
) -> Result<()> {
    match interaction {
        Either::Left(component_interaction) => {
            info!(
                "{} triggered component interaction: '{}'",
                component_interaction.user.name, component_interaction.data.custom_id
            );
            component(ctx, component_interaction, action).await
        }
        Either::Right(modal_interaction) => {
            info!(
                "{} triggered modal interaction: '{}'",
                modal_interaction.user.name, modal_interaction.data.custom_id
            );
            modal(ctx, modal_interaction, action).await
        }
    }
}

const INSTRUCTIONS_GIF: &str = "https://media.discordapp.net/attachments/997523360802152509/1420836913622810725/link_102410.gif";

async fn component(
    ctx: &SerenityContext,
    interaction: &ComponentInteraction,
    mut action: impl Iterator<Item = &str>,
) -> Result<()> {
    let data = ctx.data::<BotData>();
    let db = &data.db_handle;

    match action.next().unwrap_or_default() {
        "begin" => {
            let linked_user = db
                .request(GetLinkedUserByDiscord {
                    discord: interaction.user.id,
                })
                .await??;

            if let Some(uuid) = linked_user.map(|u| u.mc_uuid) {
                interaction.defer_ephemeral(ctx.http()).await?;

                let guild_member = interaction
                    .member
                    .as_ref()
                    .context("Interaction was triggered outside of a guild")?;

                let role_status =
                    crate::role::request::update_roles(ctx, &uuid, guild_member).await?;

                let container = role_status.to_diff_message(None);

                let message = CreateInteractionResponseFollowup::new()
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(vec![container])
                    .ephemeral(true);

                interaction.create_followup(ctx.http(), message).await?;

                Ok(())
            } else {
                let discriminator = interaction
                    .user
                    .discriminator
                    .map_or("".to_string(), |d| format!("#{}", d.get()));
                let full_username = format!("{}{}", interaction.user.name, discriminator);

                let instruction_text = CreateComponent::TextDisplay(CreateTextDisplay::new(
                    format!("# Link Accounts
Your Discord account isn't currently linked to a Hypixel profile. Follow these steps to link your accounts:
### 1. Go to any Hypixel lobby in-game
### 2. Click on __My Profile__ 🡢 __Social Media__ 🡢 __Discord__
### 3. Paste **`{full_username}`** into All Chat")
                ));

                let confirm_button = CreateButton::new("role:request:confirm_link")
                    .emoji('🔗')
                    .label("Link Account")
                    .style(ButtonStyle::Success);
                let button_section = CreateComponent::Section(CreateSection::new(
                    vec![CreateSectionComponent::TextDisplay(CreateTextDisplay::new(
                        "### 4. Confirm username
Once you've completed the steps above, \
click this button and enter your in-game username when prompted.",
                    ))],
                    CreateSectionAccessory::Button(confirm_button),
                ));

                let instruction_image =
                    CreateComponent::MediaGallery(CreateMediaGallery::new(vec![
                        CreateMediaGalleryItem::new(CreateUnfurledMediaItem::new(INSTRUCTIONS_GIF)),
                    ]));

                let container = CreateComponent::Container(CreateContainer::new(vec![
                    instruction_text,
                    instruction_image,
                    button_section,
                ]));

                let message = CreateInteractionResponseMessage::new()
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(vec![container])
                    .ephemeral(true);

                interaction
                    .create_response(ctx.http(), CreateInteractionResponse::Message(message))
                    .await?;

                Ok(())
            }
        }
        "confirm_link" => {
            let modal = modal::RoleRequestLink::create("role:request");

            interaction
                .create_response(ctx.http(), CreateInteractionResponse::Modal(modal))
                .await?;

            Ok(())
        }
        "unlink" => {
            let uuid = action.next();

            match uuid {
                Some(uuid) => {
                    db.request(RemoveLinkedUserByMinecraft {
                        mc_uuid: uuid.to_string(),
                    })
                    .await??;
                }
                None => {
                    db.request(RemoveLinkedUserByDiscord {
                        discord: interaction.user.id,
                    })
                    .await??;
                }
            }

            let text = CreateComponent::TextDisplay(CreateTextDisplay::new(
                "## Unlinked Successfully
Your accounts were successfully unlinked. \
You will be prompted to re-link the next time you request roles.",
            ));

            let container =
                CreateComponent::Container(CreateContainer::new(vec![text]).accent_color(POSITIVE));

            let message = CreateInteractionResponseMessage::new()
                .flags(MessageFlags::IS_COMPONENTS_V2)
                .components(vec![container])
                .ephemeral(true);

            interaction
                .create_response(
                    ctx.http(),
                    CreateInteractionResponse::UpdateMessage(message),
                )
                .await?;

            Ok(())
        }
        "faq" => {
            let text = CreateComponent::TextDisplay(CreateTextDisplay::new(
                format!("## Frequently asked Questions
> ### My Bingo rank role isn't being updated.
Bingo rank detection is based on your **bingo pet's rarity**. \
If you upgrade your bingo rank during or after a bingo, \
your pet will stay the **old rarity** until you create a new profile the next month.
\n> ### The bingo profile on which I achieved Immortal is no longer at zero deaths. Can I still get the role?
*This also applies if your profile has been deleted.*
If you made **screenshots** of you running `/deathcount` and of the bingo card \
when your death count was still zero, you can **apply manually in {}**.
\n> ### The bot isn't giving me any role for the new Extreme/Secret or Network Bingo.
There might not be a role yet for the bot to hand out. \
Feel free to contact staff and remind them to create the role.
\n> ### What are the criteria for the Network Bingo roles?
For all network bingo events **before 2025**, you were required to complete **all three cards**. \
Starting from Anniversary Bingo 2025, Hypixel changed the number of cards \
and you are now required to complete **any easy and any hard** card of your choice.
-# Note: For the bot to detect most completions, you need to **claim the blackout reward** of the card.
\nIf you have any other questions or you noticed an issue with the bot, ask Staff or DM {} for bot issues.", MANUAL_ROLE_CHANNEL.mention(), BOT_MAINTAINER.mention())
            ));

            let container =
                CreateComponent::Container(CreateContainer::new(vec![text]).accent_color(BLUE));

            let message = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(vec![container])
                    .ephemeral(true),
            );
            interaction.create_response(ctx.http(), message).await?;
            Ok(())
        }
        _ => bail!("Invalid interaction: Unexpected action"),
    }
}

async fn modal(
    ctx: &SerenityContext,
    interaction: &ModalInteraction,
    mut action: impl Iterator<Item = &str>,
) -> Result<()> {
    match action.next().unwrap_or_default() {
        "confirm_link_submit" => {
            let values = modal::RoleRequestLink::validate(&interaction.data.components)?;

            interaction.defer_ephemeral(ctx.http()).await?;

            let link_status =
                crate::role::request::link_user(ctx, &interaction.user, &values.username).await?;

            let container = link_status.to_response();

            interaction
                .create_followup(
                    ctx.http(),
                    CreateInteractionResponseFollowup::new()
                        .flags(MessageFlags::IS_COMPONENTS_V2)
                        .components(vec![container])
                        .ephemeral(true),
                )
                .await?;
            Ok(())
        }
        _ => Err(anyhow!("Invalid interaction: Unexpected action")),
    }
}
