use poise::serenity_prelude::{GenericChannelId, RoleId, UserId};

// path to store the database file
pub const DB_PATH: &str = "./data/db.sqlite3";
// path to the directory containing SQL scripts for `/debug sql script`
pub const DB_SCRIPTS_DIR: &str = "./data/scripts";

// how long HoB's and role request's interactive configuration menus should stay valid for
pub const MENU_TIMEOUT_SECS: u64 = 180;

// part of role request FAQ and help text
pub const MANUAL_ROLE_CHANNEL: GenericChannelId = GenericChannelId::new(1427779262462820452);
// used in splash list functionality for fetching messages
pub const SPLASHES_CHANNEL: GenericChannelId = GenericChannelId::new(1427777326321500210);
// mentioned in splash list message
pub const TY_CHANNEL: GenericChannelId = GenericChannelId::new(1427777425906860133);
// used to detect splash messages in splash list functionality
pub const SPLASH_ROLE: RoleId = RoleId::new(1427780532586151996);
// part of error messages
pub const BOT_MAINTAINER: UserId = UserId::new(821735954128830504);
