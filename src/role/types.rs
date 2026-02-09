use std::{collections::HashSet, fmt::Display};

use anyhow::Result;
use poise::serenity_prelude::{
    ButtonStyle, CacheHttp as _, CreateButton, CreateComponent, CreateContainer,
    CreateContainerComponent, CreateSection, CreateSectionAccessory, CreateSectionComponent,
    CreateTextDisplay, EditMember, Http, Member, Mentionable as _, Permissions, Role, RoleId,
    UserId,
    colours::css::{POSITIVE, WARNING},
};

use crate::shared::types::Bingo;
use crate::{db::DbHandle, role::db::role_config::InsertRoleMapping};

#[derive(Debug, Clone)]
pub struct LinkedUser {
    pub discord: UserId,
    pub mc_uuid: String,
}

impl LinkedUser {
    pub fn new(discord: UserId, minecraft_uuid: String) -> Self {
        Self {
            discord,
            mc_uuid: minecraft_uuid,
        }
    }
}

// NOTE: careful with updating network bingo enum (other than appending), always update the stored
// bit sets in the database accordingly
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum NetworkBingo {
    Unknown = 0,
    Anniversary2023 = 1,
    Halloween2023 = 2,
    Christmas2023 = 3,
    Easter2024 = 4,
    Summer2024 = 5,
    Halloween2024 = 6,
    Anniversary2025 = 7,
}

impl Display for NetworkBingo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let human_name = match self {
            NetworkBingo::Anniversary2023 => "Anniversary Bingo 2023",
            NetworkBingo::Halloween2023 => "Halloween Bingo 2023",
            NetworkBingo::Christmas2023 => "Holiday Bingo 2023",
            NetworkBingo::Easter2024 => "Easter Bingo 2024",
            NetworkBingo::Summer2024 => "Summer Bingo 2024",
            NetworkBingo::Halloween2024 => "Halloween Bingo 2024",
            NetworkBingo::Anniversary2025 => "Anniversary Bingo 2025",
            _ => "Unknown Network Bingo",
        };
        write!(f, "{}", human_name)
    }
}

impl NetworkBingo {
    pub const ALL: [NetworkBingo; 7] = [
        NetworkBingo::Anniversary2023,
        NetworkBingo::Halloween2023,
        NetworkBingo::Christmas2023,
        NetworkBingo::Easter2024,
        NetworkBingo::Summer2024,
        NetworkBingo::Halloween2024,
        NetworkBingo::Anniversary2025,
    ];

    pub fn from_u8(id: u8) -> Self {
        match id {
            1 => Self::Anniversary2023,
            2 => Self::Halloween2023,
            3 => Self::Christmas2023,
            4 => Self::Easter2024,
            5 => Self::Summer2024,
            6 => Self::Halloween2024,
            7 => Self::Anniversary2025,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Default)]
pub struct RolePatterns {
    pub completions: Option<String>,
    pub specific_completion: Option<String>,
    pub bingo_rank: Option<String>,
    pub immortal: Option<String>,
}

impl RolePatterns {
    pub fn new(
        completions: String,
        specific_completion: String,
        bingo_rank: String,
        immortal: String,
    ) -> Self {
        let completions = (!completions.is_empty()).then_some(completions);
        let specific_completion = (!specific_completion.is_empty()).then_some(specific_completion);
        let bingo_rank = (!bingo_rank.is_empty()).then_some(bingo_rank);
        let immortal = (!immortal.is_empty()).then_some(immortal);

        Self {
            completions,
            specific_completion,
            bingo_rank,
            immortal,
        }
    }
}

#[derive(Clone, Copy)]
pub struct RoleMapping {
    pub kind: RoleMappingKind,
    pub role: RoleId,
}

impl RoleMapping {
    pub fn new(kind: RoleMappingKind, role: RoleId) -> Self {
        Self { kind, role }
    }

    pub fn to_list_entry(self) -> String {
        format!("- {} – {}", self.role.mention(), self.kind)
    }

    pub fn to_section_delete(self, id_prefix: &str) -> CreateContainerComponent<'static> {
        let text =
            CreateSectionComponent::TextDisplay(CreateTextDisplay::new(self.to_list_entry()));
        let delete_button = CreateSectionAccessory::Button(
            CreateButton::new(format!("{id_prefix}:delete_mapping:{}", self.role.get()))
                .label("Delete")
                .style(ButtonStyle::Danger),
        );
        CreateContainerComponent::Section(CreateSection::new(vec![text], delete_button))
    }
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RoleMappingKindRaw {
    Completions = 0,
    SpecificCompletion = 1,
    BingoRank = 2,
    Immortal = 3,
    NetworkBingo = 4,
}

#[derive(Clone, Copy, Debug)]
pub enum RoleMappingKind {
    Completions { count: usize },
    SpecificCompletion { bingo: Bingo },
    BingoRank { rank: u8 },
    Immortal,
    NetworkBingo { bingo: NetworkBingo },
}

impl Display for RoleMappingKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RoleMappingKind::Completions { count } => write!(f, "{count} Blackouts"),
            RoleMappingKind::SpecificCompletion { bingo } => write!(f, "{bingo} Completion"),
            RoleMappingKind::BingoRank { rank } => write!(f, "Bingo Rank {rank}"),
            RoleMappingKind::Immortal => write!(f, "Immortal Role"),
            RoleMappingKind::NetworkBingo { bingo } => {
                write!(f, "{} Completion", bingo)
            }
        }
    }
}

#[derive(Debug)]
pub enum BingoRole {
    Id(RoleId),
    Name { name: String, kind: RoleMappingKind },
}

#[derive(Debug)]
pub struct RoleDelta {
    pub add: Vec<BingoRole>,
    pub remove: Vec<RoleId>,
}

impl RoleDelta {
    pub fn merge(&mut self, mut other: RoleDelta) {
        self.add.append(&mut other.add);
        self.remove.append(&mut other.remove);
    }

    pub async fn resolve(self, db: &DbHandle, roles: Vec<Role>) -> Result<RoleDeltaResolved> {
        let mut role_ids: Vec<RoleId> = Vec::new();
        for add_role in self.add {
            match add_role {
                BingoRole::Id(id) => role_ids.push(id),
                BingoRole::Name { name, kind } => {
                    if let Some(id) = roles.iter().find_map(|role| {
                        // enforces empty permissions on automatic role detection, to fully prevent
                        // accidentally granting permissions to users
                        (role.name == name && role.permissions == Permissions::empty())
                            .then_some(role.id)
                    }) {
                        db.request(InsertRoleMapping {
                            role_mapping: RoleMapping::new(kind, id),
                        })
                        .await??;
                        role_ids.push(id);
                    }
                }
            }
        }

        Ok(RoleDeltaResolved {
            add: role_ids,
            remove: self.remove,
        })
    }
}

#[derive(Debug)]
pub struct RoleDeltaResolved {
    pub add: Vec<RoleId>,
    pub remove: Vec<RoleId>,
}

impl RoleDeltaResolved {
    pub fn is_empty(&self) -> bool {
        self.add.is_empty() && self.remove.is_empty()
    }

    pub async fn apply(&self, http: &Http, member: &Member) -> Result<()> {
        let mut user_roles: HashSet<RoleId> = member.roles.iter().copied().collect();

        user_roles.extend(&self.add);
        for role in &self.remove {
            user_roles.remove(role);
        }

        member
            .guild_id
            .edit_member(
                http.http(),
                member.user.id,
                EditMember::new().roles(user_roles.into_iter().collect::<Vec<_>>()),
            )
            .await?;
        Ok(())
    }
}

pub enum LinkStatus {
    NoDiscord,
    DifferentDiscord { other_discord: String },
    DuplicateMinecraft { other_username: String },
    DuplicateDiscord { uuid: String, other_discord: UserId },
    Success,
}

impl LinkStatus {
    pub fn to_response(&self) -> CreateComponent<'static> {
        match self {
            LinkStatus::NoDiscord => {
                let text = CreateContainerComponent::TextDisplay(CreateTextDisplay::new(
                    "## No Discord account found
**Unable to find a linked Discord account in your Hypixel profile.**
\nPlease check your username spelling and try again.
-# Note: Hypixel's Discord setting usually updates instantly.",
                ));

                CreateComponent::Container(CreateContainer::new(vec![text]).accent_color(WARNING))
            }
            LinkStatus::DifferentDiscord { other_discord, .. } => {
                let text = CreateContainerComponent::TextDisplay(CreateTextDisplay::new(format!(
                    "## Wrong Discord account found
**Your linked Discord account on Hypixel is currently set to: `{other_discord}`**
\nPlease check your username spelling and try again.
-# Note: Hypixel's Discord setting usually updates instantly."
                )));

                CreateComponent::Container(CreateContainer::new(vec![text]).accent_color(WARNING))
            }
            LinkStatus::DuplicateMinecraft { other_username } => {
                let text = CreateSectionComponent::TextDisplay(CreateTextDisplay::new(format!(
                    "## Account already linked
**Your Discord account is currently linked to `{other_username}`!**
\nUse the button to unlink your previous account, then try again.",
                )));

                let unlink_button = CreateButton::new("role:request:unlink")
                    .label("Unlink Account")
                    .style(ButtonStyle::Danger);

                let section = CreateContainerComponent::Section(CreateSection::new(
                    vec![text],
                    CreateSectionAccessory::Button(unlink_button),
                ));

                CreateComponent::Container(
                    CreateContainer::new(vec![section]).accent_color(WARNING),
                )
            }
            // NOTE: Only triggers when the correct discord account is found, but the database
            // contains an existing linking entry.
            // This makes is safe to provide an unlink button since the user has proven
            // ownership of the account
            LinkStatus::DuplicateDiscord {
                uuid,
                other_discord,
            } => {
                let text = CreateSectionComponent::TextDisplay(CreateTextDisplay::new(format!(
                    "## Account already linked
**Your Minecraft account is currently linked to {}!**
\nUse the button to unlink your previous account, then try again.",
                    other_discord.mention()
                )));

                let unlink_button = CreateButton::new(format!("role:request:unlink:{}", uuid))
                    .label("Unlink Account")
                    .style(ButtonStyle::Danger);

                let section = CreateContainerComponent::Section(CreateSection::new(
                    vec![text],
                    CreateSectionAccessory::Button(unlink_button),
                ));

                CreateComponent::Container(
                    CreateContainer::new(vec![section]).accent_color(WARNING),
                )
            }
            LinkStatus::Success => {
                let text = CreateSectionComponent::TextDisplay(CreateTextDisplay::new(
                    "## Successfully linked accounts
Your Hypixel profile was successfully linked to this Discord account.
\nPress the button to continue the role requesting process.",
                ));

                let continue_button = CreateButton::new("role:request:begin")
                    .label("Request Roles")
                    .style(ButtonStyle::Primary);

                let section = CreateContainerComponent::Section(CreateSection::new(
                    vec![text],
                    CreateSectionAccessory::Button(continue_button),
                ));

                CreateComponent::Container(
                    CreateContainer::new(vec![section]).accent_color(POSITIVE),
                )
            }
        }
    }
}
