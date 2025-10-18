use std::sync::Arc;

use anyhow::Context as _;
use anyhow::Result;
use poise::serenity_prelude::{Role, RoleId, UserId, small_fixed_array::FixedArray};
use rusqlite::Connection;
use tokio::sync::oneshot;

use crate::db::{DbHandle, DbRequest};
use crate::role::{
    db,
    types::{
        BingoRole, NetworkBingo, RoleDelta, RoleMapping, RoleMappingKind, RoleMappingKindRaw,
        RolePatterns,
    },
};
use crate::shared::types::{Bingo, BitSet};

pub mod cache;
pub mod link;
pub mod role_config;

pub fn initialise_tables(conn: &mut Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "
        -- Primarily stores patterns to attempt to auto-fetch roles if not defined explicitly,
        -- as well as role IDs if there is only one associated role (immortal)
        CREATE TABLE IF NOT EXISTS role_config_global (
            id INTEGER PRIMARY KEY CHECK (id = 1),
             -- Available placeholders:
             --   `{count}` = `5` (number of completions)
            completion_pattern TEXT DEFAULT 'Blackouts: {count}',
             -- Available placeholders:
             --   `{kind}` = `Extreme ` (empty if normal)
             --   `{number}` = `2` (kind-specific ID)
            special_completion_pattern TEXT DEFAULT '{kind}Bingo #{number} Blackout',
             -- Available placeholders:
             --   `{rank}` = `4` (bingo rank)
            bingo_rank_pattern TEXT DEFAULT 'Bingo Rank {rank}',
            immortal_pattern TEXT DEFAULT 'Immortal',
            immortal_role INTEGER
        );

        -- Create row with default values if empty
        INSERT INTO role_config_global (id)
        SELECT 1
        WHERE NOT EXISTS (SELECT 1 FROM role_config_global WHERE id = 1);

        -- Completion count -> role ID mappings (configurable, auto-populated according to pattern)
        CREATE TABLE IF NOT EXISTS role_completions_config (
            count INTEGER PRIMARY KEY,
            role INTEGER NOT NULL
        );

        -- Bingo ID -> role ID mappings (configurable, auto-populated according to pattern)
        CREATE TABLE IF NOT EXISTS role_specific_completion_config (
            kind_specific_id INTEGER NOT NULL,
            bingo_kind INTEGER NOT NULL,
            role INTEGER NOT NULL,
            PRIMARY KEY(kind_specific_id, bingo_kind)
        );

        -- Bingo rank -> role ID mappings (configurable, auto-populated according to pattern)
        CREATE TABLE IF NOT EXISTS role_bingo_rank_config (
            rank INTEGER PRIMARY KEY,
            role INTEGER NOT NULL
        );

        -- Network Bingo ID -> role ID mappings (configurable)
        CREATE TABLE IF NOT EXISTS role_network_bingo_config (
            id INTEGER PRIMARY KEY,
            role INTEGER NOT NULL
        );

        -- Linked user accounts
        CREATE TABLE IF NOT EXISTS role_users_linked (
            discord_id INTEGER PRIMARY KEY,
            minecraft_uuid TEXT NOT NULL,
            UNIQUE(minecraft_uuid)
        );

        -- Cached bingo completions
        CREATE TABLE IF NOT EXISTS role_completions_cache (
            uuid TEXT PRIMARY KEY,
            updated_after_bingo INTEGER NOT NULL,
            bingo_set BLOB
        );

        -- Cached bingo rank
        CREATE TABLE IF NOT EXISTS role_bingo_rank_cache (
            uuid TEXT PRIMARY KEY,
            updated_after_bingo INTEGER NOT NULL,
            rank INTEGER NOT NULL
        );

        -- Cached immortal status
        -- Note: immortal role is never revoked
        CREATE TABLE IF NOT EXISTS role_immortal_cache (
            uuid TEXT PRIMARY KEY,
            updated_after_bingo INTEGER NOT NULL,
            has_achieved INTEGER NOT NULL
        );

        -- Cached network bingo completions
        CREATE TABLE IF NOT EXISTS role_network_bingo_cache (
            uuid TEXT PRIMARY KEY,
            updated_after_bingo INTEGER NOT NULL,
            bingo_set BLOB
        );
        ",
    )
}

impl DbHandle {
    pub async fn insert_linked_user(
        &self,
        discord_id: UserId,
        mc_uuid: String,
    ) -> Result<(Option<UserId>, Option<String>)> {
        self.dispatch_request(|response_tx| RoleDb::InsertLinkedUser {
            response_tx,
            discord_id: discord_id.get(),
            mc_uuid,
        })
        .await?
        .map(|(d, m)| (d.map(UserId::new), m))
    }

    pub async fn get_linked_user_by_discord(&self, discord_id: UserId) -> Result<Option<String>> {
        self.dispatch_request(|response_tx| RoleDb::GetLinkedUserByDiscord {
            response_tx,
            discord_id: discord_id.get(),
        })
        .await?
    }

    pub async fn get_linked_user_by_uuid(&self, mc_uuid: String) -> Result<Option<UserId>> {
        self.dispatch_request(|response_tx| RoleDb::GetLinkedUserByUuid {
            response_tx,
            mc_uuid,
        })
        .await?
        .map(|d| d.map(UserId::new))
    }

    pub async fn update_linked_user(&self, discord_id: UserId, mc_uuid: String) -> Result<()> {
        self.dispatch_request(|response_tx| RoleDb::UpdateLinkedUser {
            response_tx,
            discord_id: discord_id.get(),
            mc_uuid,
        })
        .await?
    }

    pub async fn remove_linked_user_by_discord(
        &self,
        discord_id: UserId,
    ) -> Result<Option<String>> {
        self.dispatch_request(|response_tx| RoleDb::RemoveLinkedUserByDiscord {
            response_tx,
            discord_id: discord_id.get(),
        })
        .await?
    }

    pub async fn remove_linked_user_by_uuid(&self, mc_uuid: String) -> Result<Option<UserId>> {
        self.dispatch_request(|response_tx| RoleDb::RemoveLinkedUserByUuid {
            response_tx,
            mc_uuid,
        })
        .await?
        .map(|d| d.map(UserId::new))
    }

    pub async fn detect_relevant_roles(&self, roles: Vec<Role>) -> Result<Vec<RoleMapping>> {
        self.dispatch_request(|response_tx| RoleDb::DetectRelevantRoles { response_tx, roles })
            .await?
    }

    pub async fn get_role_patterns(&self) -> Result<RolePatterns> {
        self.dispatch_request(|response_tx| RoleDb::GetRolePatterns { response_tx })
            .await?
    }

    pub async fn set_role_patterns(&self, patterns: RolePatterns) -> Result<()> {
        self.dispatch_request(|response_tx| RoleDb::SetRolePatterns {
            response_tx,
            patterns,
        })
        .await?
    }

    pub async fn get_role_mappings_by_kind(
        &self,
        kind: RoleMappingKindRaw,
    ) -> Result<Vec<RoleMapping>> {
        self.dispatch_request(|response_tx| RoleDb::GetRoleMappingsByKind { response_tx, kind })
            .await?
    }

    #[allow(dead_code)]
    pub async fn get_role(&self, kind: RoleMappingKind) -> Result<Option<RoleId>> {
        self.dispatch_request(|response_tx| RoleDb::GetRole { response_tx, kind })
            .await?
    }

    pub async fn get_roles_from_bingos(
        &self,
        user_roles: Arc<FixedArray<RoleId>>,
        bingos: Vec<Bingo>,
    ) -> Result<RoleDelta> {
        self.dispatch_request(|response_tx| RoleDb::GetRolesFromBingos {
            response_tx,
            user_roles,
            bingos,
        })
        .await?
    }

    pub async fn get_roles_from_network_bingos(
        &self,
        user_roles: Arc<FixedArray<RoleId>>,
        bingos: Vec<NetworkBingo>,
    ) -> Result<RoleDelta> {
        self.dispatch_request(|response_tx| RoleDb::GetRolesFromNetworkBingos {
            response_tx,
            user_roles,
            bingos,
        })
        .await?
    }

    pub async fn get_roles_bingo_rank(
        &self,
        user_roles: Arc<FixedArray<RoleId>>,
        rank: u8,
    ) -> Result<RoleDelta> {
        self.dispatch_request(|response_tx| RoleDb::GetRolesBingoRank {
            response_tx,
            user_roles,
            rank,
        })
        .await?
    }

    pub async fn get_role_immortal(
        &self,
        user_roles: Arc<FixedArray<RoleId>>,
    ) -> Result<Option<BingoRole>> {
        self.dispatch_request(|response_tx| RoleDb::GetRoleImmortal {
            response_tx,
            user_roles,
        })
        .await?
    }

    pub async fn insert_role_mapping(&self, role: RoleId, kind: RoleMappingKind) -> Result<()> {
        self.dispatch_request(|response_tx| RoleDb::InsertRoleMapping {
            response_tx,
            role,
            kind,
        })
        .await?
    }

    pub async fn delete_role_mapping_by_role(&self, role: RoleId) -> Result<()> {
        self.dispatch_request(|response_tx| RoleDb::DeleteRoleMappingByRole { response_tx, role })
            .await?
    }

    pub async fn cache_lookup_completions(
        &self,
        mc_uuid: String,
        current_bingo: u8,
    ) -> Result<Option<BitSet>> {
        self.dispatch_request(|response_tx| RoleDb::CacheLookupCompletions {
            response_tx,
            mc_uuid,
            current_bingo,
        })
        .await?
    }

    pub async fn cache_lookup_network_completions(
        &self,
        mc_uuid: String,
        current_bingo: u8,
    ) -> Result<Option<BitSet>> {
        self.dispatch_request(|response_tx| RoleDb::CacheLookupNetworkCompletions {
            response_tx,
            mc_uuid,
            current_bingo,
        })
        .await?
    }

    pub async fn cache_lookup_bingo_rank(
        &self,
        mc_uuid: String,
        current_bingo: u8,
    ) -> Result<Option<u8>> {
        self.dispatch_request(|response_tx| RoleDb::CacheLookupBingoRank {
            response_tx,
            mc_uuid,
            current_bingo,
        })
        .await?
    }

    pub async fn cache_lookup_immortal(
        &self,
        mc_uuid: String,
        current_bingo: u8,
    ) -> Result<Option<bool>> {
        self.dispatch_request(|response_tx| RoleDb::CacheLookupImmortal {
            response_tx,
            mc_uuid,
            current_bingo,
        })
        .await?
    }

    pub async fn cache_insert_completions(
        &self,
        mc_uuid: String,
        current_bingo: u8,
        completions: BitSet,
    ) -> Result<()> {
        self.dispatch_request(|response_tx| RoleDb::CacheInsertCompletions {
            response_tx,
            mc_uuid,
            current_bingo,
            completions,
        })
        .await?
    }

    pub async fn cache_insert_network_completions(
        &self,
        mc_uuid: String,
        current_bingo: u8,
        completions: BitSet,
    ) -> Result<()> {
        self.dispatch_request(|response_tx| RoleDb::CacheInsertNetworkCompletions {
            response_tx,
            mc_uuid,
            current_bingo,
            completions,
        })
        .await?
    }

    pub async fn cache_insert_bingo_rank(
        &self,
        mc_uuid: String,
        current_bingo: u8,
        rank: u8,
    ) -> Result<()> {
        self.dispatch_request(|response_tx| RoleDb::CacheInsertBingoRank {
            response_tx,
            mc_uuid,
            current_bingo,
            rank,
        })
        .await?
    }

    pub async fn cache_insert_immortal(
        &self,
        mc_uuid: String,
        current_bingo: u8,
        has_achieved: bool,
    ) -> Result<()> {
        self.dispatch_request(|response_tx| RoleDb::CacheInsertImmortal {
            response_tx,
            mc_uuid,
            current_bingo,
            has_achieved,
        })
        .await?
    }
}

pub enum RoleDb {
    InsertLinkedUser {
        response_tx: oneshot::Sender<Result<(Option<u64>, Option<String>)>>,
        discord_id: u64,
        mc_uuid: String,
    },
    GetLinkedUserByDiscord {
        response_tx: oneshot::Sender<Result<Option<String>>>,
        discord_id: u64,
    },
    GetLinkedUserByUuid {
        response_tx: oneshot::Sender<Result<Option<u64>>>,
        mc_uuid: String,
    },
    UpdateLinkedUser {
        response_tx: oneshot::Sender<Result<()>>,
        discord_id: u64,
        mc_uuid: String,
    },
    RemoveLinkedUserByDiscord {
        response_tx: oneshot::Sender<Result<Option<String>>>,
        discord_id: u64,
    },
    RemoveLinkedUserByUuid {
        response_tx: oneshot::Sender<Result<Option<u64>>>,
        mc_uuid: String,
    },
    DetectRelevantRoles {
        response_tx: oneshot::Sender<Result<Vec<RoleMapping>>>,
        roles: Vec<Role>,
    },
    GetRolePatterns {
        response_tx: oneshot::Sender<Result<RolePatterns>>,
    },
    SetRolePatterns {
        response_tx: oneshot::Sender<Result<()>>,
        patterns: RolePatterns,
    },
    GetRoleMappingsByKind {
        response_tx: oneshot::Sender<Result<Vec<RoleMapping>>>,
        kind: RoleMappingKindRaw,
    },
    GetRole {
        response_tx: oneshot::Sender<Result<Option<RoleId>>>,
        kind: RoleMappingKind,
    },
    GetRolesFromBingos {
        response_tx: oneshot::Sender<Result<RoleDelta>>,
        user_roles: Arc<FixedArray<RoleId>>,
        bingos: Vec<Bingo>,
    },
    GetRolesFromNetworkBingos {
        response_tx: oneshot::Sender<Result<RoleDelta>>,
        user_roles: Arc<FixedArray<RoleId>>,
        bingos: Vec<NetworkBingo>,
    },
    GetRolesBingoRank {
        response_tx: oneshot::Sender<Result<RoleDelta>>,
        user_roles: Arc<FixedArray<RoleId>>,
        rank: u8,
    },
    GetRoleImmortal {
        response_tx: oneshot::Sender<Result<Option<BingoRole>>>,
        user_roles: Arc<FixedArray<RoleId>>,
    },
    CacheLookupCompletions {
        response_tx: oneshot::Sender<Result<Option<BitSet>>>,
        mc_uuid: String,
        current_bingo: u8,
    },
    CacheLookupNetworkCompletions {
        response_tx: oneshot::Sender<Result<Option<BitSet>>>,
        mc_uuid: String,
        current_bingo: u8,
    },
    CacheLookupBingoRank {
        response_tx: oneshot::Sender<Result<Option<u8>>>,
        mc_uuid: String,
        current_bingo: u8,
    },
    CacheLookupImmortal {
        response_tx: oneshot::Sender<Result<Option<bool>>>,
        mc_uuid: String,
        current_bingo: u8,
    },
    CacheInsertCompletions {
        response_tx: oneshot::Sender<Result<()>>,
        mc_uuid: String,
        current_bingo: u8,
        completions: BitSet,
    },
    CacheInsertNetworkCompletions {
        response_tx: oneshot::Sender<Result<()>>,
        mc_uuid: String,
        current_bingo: u8,
        completions: BitSet,
    },
    CacheInsertBingoRank {
        response_tx: oneshot::Sender<Result<()>>,
        mc_uuid: String,
        current_bingo: u8,
        rank: u8,
    },
    CacheInsertImmortal {
        response_tx: oneshot::Sender<Result<()>>,
        mc_uuid: String,
        current_bingo: u8,
        has_achieved: bool,
    },
    InsertRoleMapping {
        response_tx: oneshot::Sender<Result<()>>,
        role: RoleId,
        kind: RoleMappingKind,
    },
    DeleteRoleMappingByRole {
        response_tx: oneshot::Sender<Result<()>>,
        role: RoleId,
    },
}

impl DbRequest for RoleDb {
    fn execute(self: Box<Self>, conn: &mut Connection) {
        match *self {
            RoleDb::InsertLinkedUser {
                response_tx,
                discord_id,
                mc_uuid,
            } => {
                let result = db::link::insert_linked_user(conn, discord_id, &mc_uuid)
                    .context("Failed to insert linked user");
                let _ = response_tx.send(result);
            }
            RoleDb::GetLinkedUserByDiscord {
                response_tx,
                discord_id,
            } => {
                let result = db::link::get_linked_user_by_discord(conn, discord_id)
                    .context("Failed to fetch linked user's discord user ID");
                let _ = response_tx.send(result);
            }
            RoleDb::GetLinkedUserByUuid {
                response_tx,
                mc_uuid,
            } => {
                let result = db::link::get_linked_user_by_uuid(conn, &mc_uuid)
                    .context("Failed to fetch linked user's discord user ID");
                let _ = response_tx.send(result);
            }
            RoleDb::UpdateLinkedUser {
                response_tx,
                discord_id,
                mc_uuid,
            } => {
                let result = db::link::update_linked_user(conn, discord_id, &mc_uuid)
                    .context("Failed to update linked user in database");
                let _ = response_tx.send(result);
            }
            RoleDb::RemoveLinkedUserByDiscord {
                response_tx,
                discord_id,
            } => {
                let result = db::link::remove_linked_user_by_discord(conn, discord_id)
                    .context("Failed to remove linked user from database (by Discord ID)");
                let _ = response_tx.send(result);
            }
            RoleDb::RemoveLinkedUserByUuid {
                response_tx,
                mc_uuid,
            } => {
                let result = db::link::remove_linked_user_by_uuid(conn, &mc_uuid)
                    .context("Failed to remove linked user from database (by UUID)");
                let _ = response_tx.send(result);
            }
            RoleDb::DetectRelevantRoles { response_tx, roles } => {
                let result = db::role_config::write::detect_roles(conn, &roles)
                    .context("Failed to detect and store relevant roles to database");
                let _ = response_tx.send(result);
            }
            RoleDb::GetRolePatterns { response_tx } => {
                let result = db::role_config::read::get_role_patterns(conn)
                    .context("Failed to fetch role name patterns");
                let _ = response_tx.send(result);
            }
            RoleDb::SetRolePatterns {
                response_tx,
                patterns,
            } => {
                let result = db::role_config::write::set_role_patterns(conn, &patterns)
                    .context("Failed to set role name patterns");
                let _ = response_tx.send(result);
            }
            RoleDb::GetRoleMappingsByKind { response_tx, kind } => {
                let result = db::role_config::read::mappings_by_kind(conn, &kind)
                    .context("Failed to fetch role mappings by category");
                let _ = response_tx.send(result);
            }
            RoleDb::GetRole { response_tx, kind } => {
                let result =
                    db::role_config::read::role(conn, &kind).context("Failed to fetch role");
                let _ = response_tx.send(result);
            }
            RoleDb::GetRolesFromBingos {
                response_tx,
                user_roles,
                bingos,
            } => {
                let result = db::role_config::read::roles_from_bingos(conn, bingos, user_roles)
                    .context("Failed to fetch Discord roles for completed bingos from database");
                let _ = response_tx.send(result);
            }
            RoleDb::GetRolesFromNetworkBingos {
                response_tx,
                user_roles,
                bingos,
            } => {
                let result = db::role_config::read::roles_from_network_bingos(
                    conn, &bingos, user_roles,
                )
                .context(
                    "Failed to fetch Discord roles for completed network bingos from database",
                );
                let _ = response_tx.send(result);
            }
            RoleDb::GetRolesBingoRank {
                response_tx,
                user_roles,
                rank,
            } => {
                let result = db::role_config::read::role_bingo_rank(conn, rank, user_roles)
                    .context("Failed to fetch Discord roles for bingo rank from database");
                let _ = response_tx.send(result);
            }
            RoleDb::GetRoleImmortal {
                response_tx,
                user_roles,
            } => {
                let result = db::role_config::read::role_immortal(conn, user_roles)
                    .context("Failed to fetch Immortal Discord role from database");
                let _ = response_tx.send(result);
            }
            RoleDb::CacheLookupCompletions {
                response_tx,
                mc_uuid,
                current_bingo,
            } => {
                let result = db::cache::read::cached_completions(conn, &mc_uuid, current_bingo)
                    .context("Failed to look up cached bingo completions");
                let _ = response_tx.send(result);
            }
            RoleDb::CacheLookupNetworkCompletions {
                response_tx,
                mc_uuid,
                current_bingo,
            } => {
                let result = db::cache::read::cached_network_bingos(conn, &mc_uuid, current_bingo)
                    .context("Failed to look up cached network bingo completions");
                let _ = response_tx.send(result);
            }
            RoleDb::CacheLookupBingoRank {
                response_tx,
                mc_uuid,
                current_bingo,
            } => {
                let result = db::cache::read::cached_bingo_rank(conn, &mc_uuid, current_bingo)
                    .context("Failed to look up cached bingo rank");
                let _ = response_tx.send(result);
            }
            RoleDb::CacheLookupImmortal {
                response_tx,
                mc_uuid,
                current_bingo,
            } => {
                let result = db::cache::read::cached_immortal(conn, &mc_uuid, current_bingo)
                    .context("Failed to look up cached Immortal status");
                let _ = response_tx.send(result);
            }
            RoleDb::CacheInsertCompletions {
                response_tx,
                mc_uuid,
                current_bingo,
                completions,
            } => {
                let result = db::cache::write::cache_completions(
                    conn,
                    &mc_uuid,
                    current_bingo,
                    &completions,
                )
                .context("Failed to cache completions");
                let _ = response_tx.send(result);
            }
            RoleDb::CacheInsertNetworkCompletions {
                response_tx,
                mc_uuid,
                current_bingo,
                completions,
            } => {
                let result = db::cache::write::cache_network_bingos(
                    conn,
                    &mc_uuid,
                    current_bingo,
                    &completions,
                )
                .context("Failed to cache network bingo completions");
                let _ = response_tx.send(result);
            }
            RoleDb::CacheInsertBingoRank {
                response_tx,
                mc_uuid,
                current_bingo,
                rank,
            } => {
                let result =
                    db::cache::write::cache_bingo_rank(conn, &mc_uuid, current_bingo, rank)
                        .context("Failed to cache bingo rank");
                let _ = response_tx.send(result);
            }
            RoleDb::CacheInsertImmortal {
                response_tx,
                mc_uuid,
                current_bingo,
                has_achieved,
            } => {
                let result =
                    db::cache::write::cache_immortal(conn, &mc_uuid, current_bingo, has_achieved)
                        .context("Failed to cache Immortal status");
                let _ = response_tx.send(result);
            }
            RoleDb::InsertRoleMapping {
                response_tx,
                role,
                kind,
            } => {
                let result = db::role_config::write::insert_role_mapping(conn, &role, &kind)
                    .context("Failed to store role mapping in database");
                let _ = response_tx.send(result);
            }
            RoleDb::DeleteRoleMappingByRole { response_tx, role } => {
                let result = db::role_config::write::delete_role_mapping_by_role(conn, &role)
                    .context("Failed to delete role mapping from database");
                let _ = response_tx.send(result);
            }
        }
    }
}
