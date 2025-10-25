use std::cmp::Ordering;

use anyhow::{Context as _, Result, anyhow};
use chrono::{Datelike as _, TimeZone as _};
use poise::{
    CreateReply,
    serenity_prelude::{
        ChunkGuildFilter, CreateAllowedMentions, CreateComponent, CreateContainer,
        CreateTextDisplay, Event, Mentionable as _, MessageFlags, Timestamp, UserId, collector,
        colours::{
            branding::YELLOW,
            css::{DANGER, POSITIVE, WARNING},
        },
        futures::StreamExt,
    },
};

use crate::error::UserError;
use crate::{config::SPLASHER_ROLE, shared::menu::generate_id};
use crate::{shared::Context, splashes};

#[poise::command(
    slash_command,
    rename = "latest-splash",
    subcommand_required,
    subcommands("latest_splash_list", "latest_splash_get"),
    required_bot_permissions = "VIEW_CHANNEL | READ_MESSAGE_HISTORY"
)]
pub async fn latest_splash(_ctx: Context<'_>) -> Result<()> {
    unreachable!("This shouldn't be possible to invoke")
}

/// Compile a list of when every splasher last splashed. Can take a few minutes due to rate limits!
#[poise::command(slash_command, rename = "list")]
async fn latest_splash_list(ctx: Context<'_>) -> Result<()> {
    ctx.defer().await?;

    let guild = ctx
        .guild_id()
        .context(UserError(anyhow!("Command invoked outside of a guild")))?;

    let nonce = generate_id().to_string();
    // request the guild's members to be chunked and sent over the shard connection
    ctx.serenity_context().chunk_guild(
        guild,
        None,
        false,
        ChunkGuildFilter::None,
        Some(nonce.clone()),
    );

    // collect GuildMembersChunk events
    let mut stream = collector::collect(ctx.serenity_context(), move |event| match event {
        Event::GuildMembersChunk(event) => {
            if let Some(chunk_nonce) = &event.nonce
                && chunk_nonce.as_str() == nonce
            {
                let is_final = event.chunk_index == event.chunk_count - 1;

                let splashers: Vec<_> = event
                    .members
                    .iter()
                    // filter inside collector to avoid cloning values
                    .filter_map(|m| m.roles.contains(&SPLASHER_ROLE).then_some(m.user.id))
                    .collect();

                Some((is_final, splashers))
            } else {
                None
            }
        }
        _ => None,
    });

    let mut splashers = Vec::new();
    while let Some((is_final, members)) = stream.next().await {
        splashers.extend(members.into_iter());
        if is_final {
            break;
        }
    }

    let last_splashes = splashes::latest_splash_batch(ctx.http(), &splashers).await?;

    let container_this =
        CreateComponent::Container(
            CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
                format!(
                    "## Individual latest splashes
Detected {} splashers.
### This month\n{}",
                    splashers.len(),
                    last_splashes
                        .iter()
                        .filter_map(|(id, &t)| (t > est_start_of_month_relative(0)).then_some(
                            format!("- {}: <t:{}:D>\n", id.mention(), t.unix_timestamp())
                        ))
                        .collect::<String>(),
                ),
            ))])
            .accent_color(POSITIVE),
        );
    let container_last = CreateComponent::Container(
        CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
            format!(
                "### Last month\n{}",
                last_splashes
                    .iter()
                    .filter_map(|(id, &t)| (t > est_start_of_month_relative(-1)
                        && t < est_start_of_month_relative(0))
                    .then_some(format!(
                        "- {}: <t:{}:D>\n",
                        id.mention(),
                        t.unix_timestamp()
                    )))
                    .collect::<String>(),
            ),
        ))])
        .accent_color(YELLOW),
    );
    let container_earlier = CreateComponent::Container(
        CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
            format!(
                "### Earlier\n{}",
                last_splashes
                    .iter()
                    .filter_map(|(id, &t)| (t < est_start_of_month_relative(-1)).then_some(
                        format!("- {}: <t:{}:D>\n", id.mention(), t.unix_timestamp())
                    ))
                    .collect::<String>(),
            ),
        ))])
        .accent_color(WARNING),
    );
    let container_unknown = CreateComponent::Container(
        CreateContainer::new(vec![CreateComponent::TextDisplay(CreateTextDisplay::new(
            format!(
                "### >6 months ago or never\n{}",
                splashers
                    .iter()
                    .filter_map(|id| (!last_splashes.contains_key(id))
                        .then_some(format!("- {}\n", id.mention())))
                    .collect::<String>()
            ),
        ))])
        .accent_color(DANGER),
    );

    let message = CreateReply::new()
        .flags(MessageFlags::IS_COMPONENTS_V2)
        .components(vec![
            container_this,
            container_last,
            container_earlier,
            container_unknown,
        ])
        .allowed_mentions(CreateAllowedMentions::new().empty_users());

    ctx.send(message).await?;
    Ok(())
}

fn est_start_of_month_relative(offset_months: i32) -> Timestamp {
    let est = chrono::FixedOffset::west_opt(5 * 3600).unwrap();
    let now = chrono::Utc::now().with_timezone(&est);
    let start_of_month = est
        .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .unwrap();
    let offset_month = match offset_months.cmp(&0) {
        Ordering::Less => start_of_month - chrono::Months::new(offset_months.abs() as u32),
        Ordering::Greater => start_of_month + chrono::Months::new(offset_months.abs() as u32),
        Ordering::Equal => start_of_month,
    };
    Timestamp::from_unix_timestamp(offset_month.timestamp()).unwrap()
}

/// View when a specific splasher's last splash was. Can take a while due to rate limits.
#[poise::command(
    slash_command,
    rename = "get",
    required_bot_permissions = "VIEW_CHANNEL | READ_MESSAGE_HISTORY"
)]
async fn latest_splash_get(ctx: Context<'_>, splasher: UserId) -> Result<()> {
    ctx.defer().await?;

    let last_splash = splashes::latest_splash(ctx.http(), splasher).await?;

    let text = match last_splash {
        Some(timestamp) => CreateTextDisplay::new(format!(
            "## Latest splash
{} last splashed on <t:{}:D>.",
            splasher.mention(),
            timestamp.unix_timestamp()
        )),
        None => CreateTextDisplay::new(format!(
            "## Latest splash
{} last splashed more than six months ago or has never splashed.",
            splasher.mention()
        )),
    };

    let container = CreateComponent::Container(
        CreateContainer::new(vec![CreateComponent::TextDisplay(text)]).accent_color(POSITIVE),
    );

    let message = CreateReply::new()
        .flags(MessageFlags::IS_COMPONENTS_V2)
        .components(vec![container])
        .allowed_mentions(CreateAllowedMentions::new().all_users(false));

    ctx.send(message).await?;
    Ok(())
}
