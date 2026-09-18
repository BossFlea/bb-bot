use std::ops::RangeInclusive;
use std::sync::LazyLock;
use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use poise::serenity_prelude::{Mentionable as _, RoleId, UserId};
use regex::Regex;

use crate::config::{CHCHEST_CUSTOM_ROLE, CHCHEST_KEY_ROLE, CHCHEST_ROBOT_ROLE};
use crate::error::UserError;

pub const CHCHEST_COOLDOWN: Duration = Duration::from_secs(45);

pub const X_RANGE: RangeInclusive<i32> = 202..=824;
pub const Y_RANGE: RangeInclusive<i32> = 30..=190;
pub const Z_RANGE: RangeInclusive<i32> = 202..=824;

static COORDS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:x:)?\s*(?P<x>-?\d+),?\s*(?:y:)?\s*(?P<y>-?\d+),?\s*(?:z:)?\s*(?P<z>-?\d+)$")
        .expect("regex should compile")
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    ElectronTransmitter,
    Ftx3070,
    RobotronReflector,
    SuperliteMotor,
    ControlSwitch,
    SyntheticHeart,
    KeyGuardian,
    Custom,
}

impl ItemKind {
    pub fn from_slug(slug: &str) -> Result<Self> {
        match slug {
            "electron_transmitter" => Ok(Self::ElectronTransmitter),
            "ftx_3070" => Ok(Self::Ftx3070),
            "robotron_reflector" => Ok(Self::RobotronReflector),
            "superlite_motor" => Ok(Self::SuperliteMotor),
            "control_switch" => Ok(Self::ControlSwitch),
            "synthetic_heart" => Ok(Self::SyntheticHeart),
            "key_guardian" => Ok(Self::KeyGuardian),
            "custom" => Ok(Self::Custom),
            _ => Err(anyhow!("Unknown chest content: '{slug}'")),
        }
    }

    pub fn display(self) -> &'static str {
        match self {
            Self::ElectronTransmitter => "Electron Transmitter",
            Self::Ftx3070 => "FTX 3070",
            Self::RobotronReflector => "Robotron Reflector",
            Self::SuperliteMotor => "Superlite Motor",
            Self::ControlSwitch => "Control Switch",
            Self::SyntheticHeart => "Synthetic Heart",
            Self::KeyGuardian => "Key Guardian",
            Self::Custom => "CH Chest",
        }
    }

    pub fn is_custom(self) -> bool {
        self == Self::Custom
    }

    pub fn ping_role(self) -> RoleId {
        match self {
            Self::ElectronTransmitter
            | Self::Ftx3070
            | Self::RobotronReflector
            | Self::SuperliteMotor
            | Self::ControlSwitch
            | Self::SyntheticHeart => CHCHEST_ROBOT_ROLE,
            Self::KeyGuardian => CHCHEST_KEY_ROLE,
            Self::Custom => CHCHEST_CUSTOM_ROLE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coords {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Coords {
    pub fn parse(input: &str) -> Result<Self> {
        let input = input.trim();

        let captures = COORDS_REGEX.captures(input).ok_or_else(|| {
            UserError(anyhow!(
                "Could not parse coordinates '{input}'. One of the accepted formats: `x, y, z` (e.g. `123, 64, -456`)"
            ))
        })?;

        let parse_axis = |name: &str| -> Result<i32> {
            captures
                .name(name)
                .expect("capture group should exist")
                .as_str()
                .parse()
                .map_err(|_| UserError(anyhow!("Invalid {name} coordinate in '{input}'")).into())
        };

        let coords = Self {
            x: parse_axis("x")?,
            y: parse_axis("y")?,
            z: parse_axis("z")?,
        };

        coords.validate(input)?;

        Ok(coords)
    }

    fn validate(self, input: &str) -> Result<()> {
        if !X_RANGE.contains(&self.x) || !Y_RANGE.contains(&self.y) || !Z_RANGE.contains(&self.z) {
            bail!(UserError(anyhow!(
                "Coordinates '{input}' are out of range for the Crystal Hollows world. Expected x in {:?}, y in {:?}, z in {:?}. Please double check your coordinates",
                X_RANGE,
                Y_RANGE,
                Z_RANGE,
            )));
        }
        Ok(())
    }
}

impl std::fmt::Display for Coords {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {} {}", self.x, self.y, self.z)
    }
}

pub const DEFAULT_CONTACT: &str = "Send your IGN in the thread below";

pub fn announcement_text(
    kind: ItemKind,
    custom_item: Option<&str>,
    coords: Coords,
    reporter: UserId,
    contact: Option<&str>,
    notes: Option<&str>,
) -> String {
    let item_display = match (kind.is_custom(), custom_item) {
        (true, Some(item)) => item.to_string(),
        _ => kind.display().to_string(),
    };

    let contact_display = contact.unwrap_or(DEFAULT_CONTACT);

    let mut text = format!(
        "## {} found:  {item_display}
### Coordinates: **`{coords}`**
### Instructions
{contact_display}",
        reporter.mention(),
    );

    if let Some(notes) = notes {
        text.push_str(&format!("\n### Additional notes\n{notes}"));
    }

    text
}

const THREAD_TITLE: &str = " reported by ";

pub fn thread_name(item_display: &str, username: &str) -> String {
    let username_chars = username.chars().count();
    let title_chars = THREAD_TITLE.chars().count();

    let mut title: String = item_display
        .chars()
        .take(100 - username_chars - title_chars)
        .collect();
    title.push_str(THREAD_TITLE);
    title.push_str(username);
    title
}

const CLOSED_MARKER: &str = "NO LONGER AVAILABLE";

pub fn mark_closed(old: &str) -> Result<String> {
    Ok(match old.split_once('\n') {
        Some((title, rest)) => format!("~~{title}~~ {CLOSED_MARKER}\n{rest}"),
        None => format!("~~{old}~~\n{CLOSED_MARKER}"),
    })
}
