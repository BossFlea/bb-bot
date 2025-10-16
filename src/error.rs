use anyhow::{Error, Result, anyhow};
use poise::{
    BoxFuture, CreateReply, FrameworkError,
    serenity_prelude::{
        CacheHttp as _, Context as SerenityContext, CreateComponent, CreateContainer,
        CreateInteractionResponse, CreateInteractionResponseFollowup,
        CreateInteractionResponseMessage, CreateTextDisplay, FullEvent, Interaction,
        Mentionable as _, MessageFlags,
        colours::css::{DANGER, WARNING},
    },
};
use thiserror::Error;
use tracing::{error, warn};

use crate::config::BOT_MAINTAINER;

#[derive(Error, Debug)]
#[error(transparent)]
pub struct UserError(#[from] pub anyhow::Error);

pub fn deduplicate_error_chain(error: &mut Error) {
    let mut error_chain: Vec<String> = error.chain().map(|err| err.to_string()).collect();

    error_chain.dedup();

    let mut error_chain = error_chain.into_iter().rev();
    let mut new_error = anyhow!(error_chain.next().unwrap());

    for message in error_chain {
        new_error = new_error.context(message);
    }

    *error = new_error;
}

fn internal_error_container(error: &Error) -> CreateComponent<'static> {
    CreateComponent::Container(
        CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
            format!(
                "## Internal Error\n```\n{error:?}\n```
Please report this to {}!",
                BOT_MAINTAINER.mention()
            ),
        ))])
        .accent_color(DANGER),
    )
}

fn user_error_container(error: &Error) -> CreateComponent<'static> {
    CreateComponent::Container(
        CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
            format!("## You seem to have made a mistake\n```\n{error:?}\n```"),
        ))])
        .accent_color(WARNING),
    )
}

pub async fn event_handler_error(mut error: Error, ctx: &SerenityContext, event: &FullEvent) {
    let container = if error.is::<UserError>() {
        deduplicate_error_chain(&mut error);
        error!("User error while handling event: {error:#}");
        user_error_container(&error)
    } else {
        deduplicate_error_chain(&mut error);
        error!("Failed to handle event {event:?}: {error:#}");
        internal_error_container(&error)
    };

    let response_message = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::default()
            .flags(MessageFlags::IS_COMPONENTS_V2)
            .components(vec![container.clone()])
            .ephemeral(true),
    );

    let followup_message = CreateInteractionResponseFollowup::new()
        .flags(MessageFlags::IS_COMPONENTS_V2)
        .components(vec![container])
        .ephemeral(true);

    match event {
        FullEvent::InteractionCreate {
            interaction: Interaction::Component(interaction),
            ..
        } => {
            if interaction
                .create_response(ctx.http(), response_message)
                .await
                .is_err()
            {
                let _ = interaction
                    .create_followup(ctx.http(), followup_message)
                    .await;
            }
        }
        FullEvent::InteractionCreate {
            interaction: Interaction::Modal(interaction),
            ..
        } => {
            if interaction
                .create_response(ctx.http(), response_message)
                .await
                .is_err()
            {
                let _ = interaction
                    .create_followup(ctx.http(), followup_message)
                    .await;
            }
        }
        _ => (),
    }
}

async fn try_handle_error<U>(error: FrameworkError<'_, U, Error>) -> Result<()>
where
    U: Send + Sync + 'static,
{
    match error {
        FrameworkError::Command { mut error, ctx, .. } => {
            let invocation_string = ctx.invocation_string();
            let container = if error.is::<UserError>() {
                deduplicate_error_chain(&mut error);
                error!("A user error occurred while executing {invocation_string:?}: {error:#}");
                user_error_container(&error)
            } else {
                deduplicate_error_chain(&mut error);
                error!("An error occurred while executing {invocation_string:?}: {error:#}");
                internal_error_container(&error)
            };

            let response_message = CreateReply::default()
                .flags(MessageFlags::IS_COMPONENTS_V2)
                .components(vec![container])
                .reply(true)
                .ephemeral(true);

            ctx.send(response_message).await?;
        }
        FrameworkError::SubcommandRequired { ctx } => {
            warn!(
                "User attempted to invoke a subcommand-only command, without a subcommand: {:?}",
                ctx.invocation_string(),
            );

            let prefix = ctx.prefix();

            let container = CreateComponent::Container(
                CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
                    format!(
                        "## Subcommand required
You must specify one of the following subcommands:\n{}",
                        ctx.command()
                            .subcommands
                            .iter()
                            .map(|subcommand| {
                                format!("- `{prefix}{}`", subcommand.qualified_name)
                            })
                            .collect::<Vec<_>>()
                            .join("\n"),
                    ),
                ))])
                .accent_color(WARNING),
            );

            ctx.send(
                CreateReply::default()
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(vec![container])
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::CommandPanic { ctx, payload, .. } => {
            if let Some(payload) = payload {
                error!(
                    "[PANIC] Invocation `{}` caused a panic with payload: {}",
                    ctx.invocation_string(),
                    payload
                );
            } else {
                error!(
                    "[PANIC] Invocation `{}` caused a panic with unknown payload",
                    ctx.invocation_string()
                );
            }

            let container = CreateComponent::Container(
                CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
                    format!(
                        "## Panicked
A critical error occurred and the command handler panicked!
-# This should not affect the bot as a whole.\n
Please report this to {}!
",
                        BOT_MAINTAINER.mention()
                    ),
                ))])
                .accent_color(DANGER),
            );

            ctx.send(
                CreateReply::default()
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(vec![container])
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::ArgumentParse {
            error, input, ctx, ..
        } => {
            let invocation_string = ctx.invocation_string();
            let description = match input {
                Some(input) => {
                    format!(
                        "Failed to parse {input:?} from `{invocation_string}` into an argument: {error}",
                    )
                }
                None => {
                    format!("Failed to parse an argument from `{invocation_string}`: {error}")
                }
            };

            warn!(description);

            let container = CreateComponent::Container(
                CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
                    format!("## Failed to parse argument\n{description}"),
                ))])
                .accent_color(WARNING),
            );

            ctx.send(
                CreateReply::default()
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(vec![container])
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::CommandStructureMismatch {
            description, ctx, ..
        } => {
            error!(
                "Mismatch between registered command and poise command for `/{}`: {description}",
                ctx.command.qualified_name,
            );
            let container = CreateComponent::Container(
                CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
                    format!(
                        "## Command structure mismatch\n```\n{description}\n```
Try re-registering the bot's commands using '{} register'",
                        ctx.framework().bot_id().mention()
                    ),
                ))])
                .accent_color(WARNING),
            );

            ctx.send(
                CreateReply::default()
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(vec![container])
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::CooldownHit {
            remaining_cooldown,
            ctx,
            ..
        } => {
            warn!("User hit cooldown with {:?}", ctx.invocation_string());
            let container = CreateComponent::Container(
                CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
                    format!(
                        "## Cooldown hit
You must wait **~{} seconds** before you can use this command again.",
                        remaining_cooldown.as_secs()
                    ),
                ))])
                .accent_color(WARNING),
            );

            ctx.send(
                CreateReply::default()
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(vec![container])
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::MissingBotPermissions {
            missing_permissions,
            ctx,
            ..
        } => {
            warn!(
                "Bot is lacking permissions for {:?}: {missing_permissions}",
                ctx.invocation_string()
            );
            let container = CreateComponent::Container(
                CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
                    format!(
                        "## Lacking Bot Permissions
The bot is missing the following permissions to execute this command: **{missing_permissions}**"
                    ),
                ))])
                .accent_color(WARNING),
            );

            ctx.send(
                CreateReply::default()
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(vec![container])
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::MissingUserPermissions {
            missing_permissions,
            ctx,
            ..
        } => {
            let description = if let Some(permissions) = missing_permissions {
                warn!(
                    "User is lacking permissions for {:?}: {permissions}",
                    ctx.invocation_string(),
                );
                format!(
                    "You are missing the following permissions to execute this command: **{permissions}**"
                )
            } else {
                warn!(
                    "User is lacking permissions for {:?}",
                    ctx.invocation_string(),
                );
                "You do not have the permissions needed to execute this command".to_string()
            };

            let container = CreateComponent::Container(
                CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
                    format!("## Lacking User Permissions\n{description}"),
                ))])
                .accent_color(WARNING),
            );

            ctx.send(
                CreateReply::default()
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(vec![container])
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::NotAnOwner { ctx, .. } => {
            warn!(
                "Non-owner attempted to invoke {:?}",
                ctx.invocation_string(),
            );

            // Don't respond to prefix commands (e.g. `register`)
            if ctx.prefix() == "/" {
                let container = CreateComponent::Container(
                    CreateContainer::new(vec![CreateComponent::TextDisplay(
                        CreateTextDisplay::new(
                            "Owner-only Command
You must be an owner to use this command.",
                        ),
                    )])
                    .accent_color(WARNING),
                );

                ctx.send(
                    CreateReply::default()
                        .flags(MessageFlags::IS_COMPONENTS_V2)
                        .components(vec![container])
                        .reply(true)
                        .ephemeral(true),
                )
                .await?;
            }
        }
        FrameworkError::GuildOnly { ctx, .. } => {
            warn!(
                "User attempted to invoke {:?} outside of a guild",
                ctx.invocation_string(),
            );

            let container = CreateComponent::Container(
                CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
                    "Server-only Command
You cannot use this command outside of a server.",
                ))])
                .accent_color(WARNING),
            );

            ctx.send(
                CreateReply::default()
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(vec![container])
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::DmOnly { ctx, .. } => {
            warn!(
                "User attempted to invoke {:?} outside of DMs",
                ctx.invocation_string(),
            );

            let container = CreateComponent::Container(
                CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
                    "DMs-only Command
You cannot use this command touside of DMs.",
                ))])
                .accent_color(WARNING),
            );

            ctx.send(
                CreateReply::default()
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(vec![container])
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::NsfwOnly { ctx, .. } => {
            warn!(
                "User attempted to invoke {:?} outside of an NSFW channel",
                ctx.invocation_string(),
            );

            let container = CreateComponent::Container(
                CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
                    "NSFW Command
You cannot use this command outside of an NSFW channel.",
                ))])
                .accent_color(WARNING),
            );

            ctx.send(
                CreateReply::default()
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(vec![container])
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::CommandCheckFailed { error, ctx, .. } => match error {
            Some(mut error) => {
                deduplicate_error_chain(&mut error);
                error!("Check errored for {:?}: {error:#}", ctx.invocation_string());

                let container = CreateComponent::Container(
                    CreateContainer::new(vec![CreateComponent::TextDisplay(
                        CreateTextDisplay::new("Failed to perform check\n```\n{error:?}\n```"),
                    )])
                    .accent_color(DANGER),
                );

                ctx.send(
                    CreateReply::default()
                        .flags(MessageFlags::IS_COMPONENTS_V2)
                        .components(vec![container])
                        .reply(true)
                        .ephemeral(true),
                )
                .await?;
            }
            None => {
                warn!("Check failed for {:?}", ctx.invocation_string());
            }
        },
        FrameworkError::DynamicPrefix { mut error, msg, .. } => {
            deduplicate_error_chain(&mut error);
            error!("Dynamic prefix failed for a message: {error:#}\n{msg:#?}");
        }
        FrameworkError::UnknownCommand {
            prefix,
            msg_content,
            ..
        } => {
            // NOTE: doesn't respond to prevent interference with foreign commands on the same bot account
            warn!("Recognized prefix {prefix:?} but did not recognize command {msg_content:?}");
        }
        FrameworkError::UnknownInteraction { interaction, .. } => {
            warn!(
                "Received interaction for an unknown command: {:?}",
                interaction.data.name,
            );
        }
        FrameworkError::PermissionFetchFailed { ctx, .. } => {
            error!("Failed to fetch permissions");

            let container = CreateComponent::Container(
                CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
                    "# Failed to fetch permissions
Failed to fetch permissions for you or the bot.",
                ))])
                .accent_color(DANGER),
            );

            ctx.send(
                CreateReply::default()
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(vec![container])
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::NonCommandMessage { mut error, msg, .. } => {
            deduplicate_error_chain(&mut error);
            error!("An error occurred in the non-command message callback: {error:#}\n{msg:#?}");
        }
    }

    Ok(())
}

pub fn error_handler<U>(error: FrameworkError<'_, U, Error>) -> BoxFuture<'_, ()>
where
    U: Send + Sync + 'static,
{
    Box::pin(async move {
        if let Err(mut err) = try_handle_error(error).await {
            deduplicate_error_chain(&mut err);
            error!("Failed to handle error: {err:#}");
        }
    })
}
