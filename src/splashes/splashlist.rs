use std::collections::HashMap;

use anyhow::Result;
use chrono::{Datelike, TimeZone};
use poise::{
    CreateReply,
    serenity_prelude::{
        CreateAttachment, CreateComponent, CreateContainer, CreateMediaGallery,
        CreateMediaGalleryItem, CreateTextDisplay, CreateUnfurledMediaItem, Mentionable as _,
        MessageFlags, Timestamp, UserId,
    },
};

use crate::config::{SPLASH_PING_ROLE, TY_CHANNEL};
use crate::shared::{Context, types::BingoKind};
use crate::splashes::fetch;

mod chart;

#[derive(Debug, Clone)]
pub struct SplashList {
    items: Vec<(Timestamp, UserId)>,
    bingo_days: usize,
}

impl SplashList {
    pub fn new(items: Vec<(Timestamp, UserId)>, bingo_days: usize) -> SplashList {
        SplashList { items, bingo_days }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn bingo_days(&self) -> usize {
        self.bingo_days
    }

    pub fn per_splasher_sorted(&self) -> Vec<(UserId, u32)> {
        let mut splashers_map = HashMap::new();

        for (_, user_id) in &self.items {
            *splashers_map.entry(*user_id).or_insert(0) += 1
        }

        let mut splashers: Vec<_> = splashers_map.into_iter().collect();

        // compare "in reverse" for descending count
        splashers.sort_by(|(_, count_a), (_, count_b)| count_b.cmp(count_a));
        splashers
    }

    pub fn split_days_top_3(&self) -> Vec<[u32; 4]> {
        let mut day_maps = vec![HashMap::new(); self.bingo_days];

        for (timestamp, user_id) in &self.items {
            let day_of_month = day_of_month_est(*timestamp) as usize;

            if day_of_month > self.bingo_days {
                continue;
            }

            *day_maps[day_of_month - 1].entry(user_id).or_insert(0) += 1;
        }

        let top_3: Vec<UserId> = self
            .per_splasher_sorted()
            .iter()
            .take(3)
            .map(|(user_id, _)| *user_id)
            .collect();

        day_maps
            .iter()
            .map(|day| {
                let mut counts = [0u32; 4];
                let mut rest = 0u32;

                for (&user, &count) in day {
                    if let Some(pos) = top_3.iter().position(|u| u == user) {
                        counts[pos] = count;
                    } else {
                        rest += count;
                    }
                }

                counts[3] = rest;
                counts
            })
            .collect()
    }
}

fn timestamp_start_of_day_est(day_of_month: u32) -> Timestamp {
    let est = chrono::FixedOffset::west_opt(5 * 3600).unwrap();
    let now = chrono::Utc::now().with_timezone(&est);
    let start_of_month = est
        .with_ymd_and_hms(now.year(), now.month(), day_of_month, 0, 0, 0)
        .unwrap();
    Timestamp::from_unix_timestamp(start_of_month.timestamp()).unwrap()
}

fn day_of_month_est(timestamp: Timestamp) -> u32 {
    let est = chrono::FixedOffset::west_opt(5 * 3600).unwrap();
    let timestamp = chrono::DateTime::from_timestamp(timestamp.unix_timestamp(), 0)
        .unwrap()
        .with_timezone(&est);
    timestamp.day()
}

fn current_month_name_est() -> String {
    let est = chrono::FixedOffset::west_opt(5 * 3600).unwrap();
    let now = chrono::Utc::now().with_timezone(&est);
    now.format("%B %Y").to_string()
}

pub async fn generate_message(ctx: &Context<'_>) -> Result<CreateReply<'static>> {
    let data = ctx.data();
    let db = &data.db_handle;
    let api = &data.api_handle;

    let (current_bingo, _) = api.update_current_bingo(db).await?;

    let bingo_days = if current_bingo.kind == BingoKind::Normal {
        7
    } else {
        14
    };

    let start_timestamp = timestamp_start_of_day_est(1);
    let end_timestamp = timestamp_start_of_day_est(bingo_days as u32 + 1);

    let mut fetcher = fetch::FetchSplashes::new();
    let splash_messages: Vec<_> = fetcher
        .splashes_during(ctx.http(), start_timestamp, end_timestamp)
        .await?
        .iter()
        .map(|m| (m.timestamp, m.author.id))
        .collect();
    let splashes = SplashList::new(splash_messages, bingo_days);

    let total_splashes = splashes.len();

    let splasher_list: String = splashes
        .per_splasher_sorted()
        .iter()
        .enumerate()
        .map(|(i, (splasher_id, count))| {
            let suffix = match i {
                0 => " (🔴)",
                1 => " (🟢)",
                2 => " (🔵)",
                _ => "",
            };
            format!("{}: **{}**{suffix}\n", splasher_id.mention(), count)
        })
        .collect();

    let hourly_average = total_splashes as f64 / (24 * bingo_days) as f64;

    let text_overview = CreateTextDisplay::new(format!(
        "
## {} Splash List
### Overview
Total Splashes: **{total_splashes}**
Hourly Average: **{hourly_average:.2}** splashes/h
### Distribution Chart
        ",
        current_month_name_est(),
    ));

    let individual_list = CreateTextDisplay::new(format!(
        "
### Individual Splashers:
{splasher_list}\
### Go thank them in {} :heart:!
||{}||
        ",
        TY_CHANNEL.mention(),
        SPLASH_PING_ROLE.mention()
    ));

    let chart_bytes =
        tokio::task::spawn_blocking(move || chart::distribution_png_bytes(&splashes)).await??;

    let chart_attachment = CreateAttachment::bytes(chart_bytes, "chart.png");

    Ok(CreateReply::new()
        .flags(MessageFlags::IS_COMPONENTS_V2)
        .components(vec![CreateComponent::Container(CreateContainer::new(
            vec![
                CreateComponent::TextDisplay(text_overview),
                CreateComponent::MediaGallery(CreateMediaGallery::new(vec![
                    CreateMediaGalleryItem::new(CreateUnfurledMediaItem::new(
                        "attachment://chart.png",
                    )),
                ])),
                CreateComponent::TextDisplay(individual_list),
            ],
        ))])
        .attachment(chart_attachment))
}
