use std::{collections::HashMap, sync::Arc};

use anyhow::Error;
use tokio::sync::Mutex;

use crate::db::DbHandle;
use crate::hob::menu::HobEditSession;
use crate::hypixel_api::ApiHandle;
use crate::role::menu::RoleConfigSession;
use crate::splash_reminder::SplashReminderHandle;

pub mod db;
pub mod interaction;
pub mod menu;
pub mod types;

pub struct BotData {
    pub db_handle: DbHandle,
    pub api_handle: ApiHandle,
    // NOTE: nested Arc-Mutexes so that commands and interactions can clone the inner Arc and drop
    // the outer lock, allowing for concurrent mutable access to separate entries
    // NOTE: outer Arc is necessary for timeout functionality, despite poise wrapping the data
    // struct in another reference-counting pointer
    pub hob_sessions: Arc<Mutex<HashMap<u64, Arc<Mutex<HobEditSession>>>>>,
    pub role_sessions: Arc<Mutex<HashMap<u64, Arc<Mutex<RoleConfigSession>>>>>,
    pub splash_reminder: Mutex<SplashReminderHandle>,
}

pub type Context<'a> = poise::Context<'a, BotData, Error>;
