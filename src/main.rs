use std::{borrow::Cow, collections::HashMap, env, str::FromStr as _, sync::Arc};

use anyhow::{Result, anyhow};
use either::Either;
use poise::{
    Framework, FrameworkOptions, PrefixFrameworkOptions,
    serenity_prelude::{
        ClientBuilder, Context as SerenityContext, EventHandler, FullEvent, GatewayIntents,
        Interaction, Mentionable as _, Permissions, Token, async_trait,
    },
};
use tokio::sync::{Mutex, mpsc};
use tracing::{info, warn};

use db::DbHandle;
use hypixel_api::ApiHandle;
use shared::BotData;

use crate::config::SPLASHES_CHANNEL;
use crate::splash_reminder::SplashReminderHandle;

mod commands;
mod config;
mod db;
mod error;
mod hob;
mod hypixel_api;
mod log;
mod role;
mod shared;
mod splash_reminder;
mod splashes;

fn get_env_var(name: &str) -> Result<String> {
    env::var(name).map_err(|err| anyhow!("Failed to load environment variable '{name}': {err:#}"))
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = log::init_log();

    _ = dotenvy::dotenv();

    let token = Token::from_str(&get_env_var("DISCORD_TOKEN")?)?;
    let api_key = get_env_var("HYPIXEL_API_KEY").unwrap_or_else(|_| {
        warn!("No Hypixel API key provided, role request functionality will not work");
        String::new()
    });

    let (db_tx, db_rx) = mpsc::channel(32);
    match db::db_thread::start_db_thread(db_rx).await {
        Ok(Ok(())) => {
            info!("Database thread has completed initialisation");
            Ok(())
        }
        Ok(Err(err)) => Err(anyhow!("Failed to initialise database thread: {err:#}")),
        Err(_) => Err(anyhow!("Database thread panicked during initialisation")),
    }?;

    let mut commands = vec![
        commands::debug::debug(),
        commands::hob::hob(),
        commands::role::rolerequest(),
        commands::splashlist::splashlist(),
        commands::lastsplashed::lastsplashed(),
        commands::register::register(),
        commands::splashreminder::splashreminder(),
        commands::register::unregister(),
        commands::baninfo::baninfo(),
    ];
    // Set default permission to `MANAGE_GUILD`, as bots cannot access endpoint for role-based
    // permission override (manual configuration intended)
    for cmd in &mut commands {
        cmd.default_member_permissions = Permissions::MANAGE_GUILD;
    }

    // `GUILD_MESSAGES`: only for `register` prefix command
    // `MESSAGE_CONTENT`: same reason, as well as for splash message fetching
    // `GUILD_MEMBERS`: needed to fetch all members with the splasher role
    // `GUILD_MESSAGE_REACTIONS`: necessary to detect reactions on splash messages
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_MEMBERS;

    let prefix_options = PrefixFrameworkOptions {
        dynamic_prefix: Some(|ctx| {
            Box::pin(async move {
                Ok(Some(Cow::Owned(
                    ctx.framework.bot_id().mention().to_string(),
                )))
            })
        }),
        ..Default::default()
    };

    let framework = Framework::builder()
        .options(FrameworkOptions {
            commands,
            prefix_options,
            on_error: error::error_handler,
            pre_command: |ctx| {
                Box::pin(async move {
                    info!(
                        "[>] `{}` invoked by {}",
                        ctx.invocation_string(),
                        ctx.author().name,
                    );
                })
            },
            post_command: |ctx| {
                Box::pin(async move {
                    info!(
                        "[<] {}'s `{}` invocation completed successfully",
                        ctx.author().name,
                        ctx.invocation_string(),
                    );
                })
            },
            ..Default::default()
        })
        .initialize_owners(false)
        .build();

    let mut client = ClientBuilder::new(token, intents)
        .framework(Box::new(framework))
        .event_handler(Arc::new(Handler))
        .data(Arc::new(BotData {
            db_handle: DbHandle::new(db_tx),
            api_handle: ApiHandle::new(api_key),
            hob_sessions: Arc::new(Mutex::new(HashMap::new())),
            role_sessions: Arc::new(Mutex::new(HashMap::new())),
            splash_reminder: Mutex::new(SplashReminderHandle::new()),
        }))
        .await?;

    client.start().await?;

    Ok(())
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn dispatch(&self, ctx: &SerenityContext, event: &FullEvent) {
        if let Err(err) = match event {
            FullEvent::InteractionCreate { interaction, .. } => match interaction {
                Interaction::Component(interaction) => {
                    let mut action = interaction.data.custom_id.split(':');

                    match action.next().unwrap_or_default() {
                        "hob" => {
                            hob::interaction::handle_interaction(
                                ctx,
                                Either::Left(interaction),
                                action,
                            )
                            .await
                        }
                        "role" => {
                            role::interaction::handle_interaction(
                                ctx,
                                Either::Left(interaction),
                                action,
                            )
                            .await
                        }
                        _ => Ok(()),
                    }
                }
                Interaction::Modal(interaction) => {
                    let mut action = interaction.data.custom_id.split(':');

                    match action.next().unwrap_or_default() {
                        "hob" => {
                            hob::interaction::handle_interaction(
                                ctx,
                                Either::Right(interaction),
                                action,
                            )
                            .await
                        }
                        "role" => {
                            role::interaction::handle_interaction(
                                ctx,
                                Either::Right(interaction),
                                action,
                            )
                            .await
                        }
                        _ => Ok(()),
                    }
                }
                _ => Ok(()),
            },
            FullEvent::Message { new_message } => {
                if new_message.channel_id == SPLASHES_CHANNEL {
                    splash_reminder::event::splashes_message(ctx, new_message).await
                } else {
                    Ok(())
                }
            }
            FullEvent::ReactionAdd { add_reaction, .. } => {
                if add_reaction.channel_id == SPLASHES_CHANNEL {
                    splash_reminder::event::splashes_reaction(ctx, add_reaction).await
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        } {
            error::event_handler_error(err, ctx, event).await;
        }
    }
}
