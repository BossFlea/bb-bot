use std::sync::Arc;
use std::time::Duration;

use crate::shared::{
    BotData,
    db::{GetCurrentBingo, GetSplashReminder},
};
use crate::splash_reminder::reminder::{self, ReminderVariant, TIMER_WAIT_SECS};
use crate::splashes::fetch::FetchSplashes;

use anyhow::Result;
use chrono::Utc;
use poise::serenity_prelude::{Context as SerenityContext, Message, Reaction, ReactionType};

pub async fn splashes_message(ctx: &SerenityContext, message: &Message) -> Result<()> {
    if !FetchSplashes::is_splash(message) {
        return Ok(());
    }

    let (enabled, _, _) = ctx
        .data::<BotData>()
        .db_handle
        .request(GetSplashReminder)
        .await??;

    if !enabled {
        return Ok(());
    }

    // abort if no active bingo in one hour
    if !is_active_bingo_with_offset(ctx, Duration::from_hours(1)).await? {
        return Ok(());
    }

    // update latest splash and initiate timer
    let data = ctx.data::<BotData>();
    {
        let mut handle = data.splash_reminder.lock().await;
        handle.new_splash(Arc::clone(&ctx.http), message.id).await;
    }

    Ok(())
}

pub async fn splashes_reaction(ctx: &SerenityContext, reaction: &Reaction) -> Result<()> {
    let data = ctx.data::<BotData>();
    {
        let mut handle = data.splash_reminder.lock().await;
        let Some(latest_id) = handle.latest() else {
            return Ok(());
        };

        // verify message id
        if reaction.message_id != latest_id {
            return Ok(());
        }

        // ensure timer wait time hasn't passed yet
        let now = Utc::now().timestamp();
        let created = latest_id.created_at().timestamp();

        if now - created >= TIMER_WAIT_SECS as i64 {
            // clear so that check isn't performed on the next reaction
            handle.clear_latest();
            return Ok(());
        }
    }

    // fetch and verify configuration
    let (enabled, emoji_id, emoji_count) = ctx
        .data::<BotData>()
        .db_handle
        .request(GetSplashReminder)
        .await??;

    if !enabled || emoji_id.is_none() {
        return Ok(());
    }
    let emoji_id = emoji_id.unwrap();
    let emoji_count = emoji_count.unwrap_or(50);

    // check for configured emoji
    let ReactionType::Custom { id: new_id, .. } = reaction.emoji else {
        return Ok(());
    };

    if new_id != emoji_id {
        return Ok(());
    }

    // abort if no active bingo
    if !is_active_bingo(ctx).await? {
        return Ok(());
    }

    let message = reaction.message(ctx).await?;
    // find and compare reaction count
    if let Some(r) = message
        .reactions
        .iter()
        .find(|r| matches!(&r.reaction_type, ReactionType::Custom { id, .. } if *id == emoji_id))
        && r.count >= emoji_count as u64
    {
        // cancel 1 hour reminder
        {
            let mut handle = data.splash_reminder.lock().await;
            handle.clear_latest();
        }
        // trigger reminder
        if let ReactionType::Custom { animated, id, name } = &r.reaction_type
            && *id == emoji_id
        {
            let emoji_mention = format!(
                "<{}:{}:{}>",
                if *animated { "a" } else { "" },
                name.as_deref().unwrap_or_default(),
                id.get()
            );
            reminder::send_reminder(
                Arc::clone(&ctx.http),
                ReminderVariant::Reactions {
                    emoji_mention,
                    emoji_count,
                },
            )
            .await?;
        }
    }

    Ok(())
}

async fn is_active_bingo(ctx: &SerenityContext) -> Result<bool> {
    is_active_bingo_with_offset(ctx, Duration::ZERO).await
}

async fn is_active_bingo_with_offset(
    ctx: &SerenityContext,
    future_offset: Duration,
) -> Result<bool> {
    let data = ctx.data::<BotData>();
    let current_bingo = data.db_handle.request(GetCurrentBingo).await??;

    let now = Utc::now().timestamp();
    let now_offset = now + future_offset.as_secs() as i64;

    let (_, start, end) = match current_bingo {
        Some((b, start, end)) if now < end => (b, start, end),
        // make sure current bingo data is up-to-date if possibly outdated
        _ => {
            data.api_handle
                .update_current_bingo(&data.db_handle)
                .await?
        }
    };

    // Only consider bingo active if it will still be active after `future_offset`
    let bingo_active = now_offset > start && now_offset < end;

    Ok(bingo_active)
}
