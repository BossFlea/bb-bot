use std::sync::Arc;

use poise::serenity_prelude::{RoleId, small_fixed_array::FixedArray};
use rusqlite::{Connection, OptionalExtension as _, Result, params};

use crate::db::DbRequest;
use crate::role::types::{
    BingoRole, NetworkBingo, RoleDelta, RoleMapping, RoleMappingKind, RoleMappingKindRaw,
    RolePatterns,
};
use crate::shared::types::{Bingo, BingoKind};

pub struct GetRoleMappingsByKind {
    pub kind: RoleMappingKindRaw,
}
impl DbRequest for GetRoleMappingsByKind {
    type ReturnValue = Result<Vec<RoleMapping>>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        match self.kind {
            RoleMappingKindRaw::Completions => {
                let mut statement =
                    conn.prepare("SELECT role, count FROM role_completions_config")?;
                statement
                    .query_map([], |row| {
                        Ok(RoleMapping::new(
                            RoleMappingKind::Completions {
                                count: row.get("count")?,
                            },
                            RoleId::new(row.get("role")?),
                        ))
                    })?
                    .collect::<Result<Vec<RoleMapping>>>()
            }
            RoleMappingKindRaw::SpecificCompletion => {
                let mut statement = conn.prepare(
                "SELECT role, kind_specific_id, bingo_kind FROM role_specific_completion_config",
            )?;
                statement
                    .query_map([], |row| {
                        Ok(RoleMapping::new(
                            RoleMappingKind::SpecificCompletion {
                                bingo: Bingo::new(
                                    row.get("kind_specific_id")?,
                                    BingoKind::from_u8(row.get("bingo_kind")?),
                                    None,
                                ),
                            },
                            RoleId::new(row.get("role")?),
                        ))
                    })?
                    .collect::<Result<Vec<RoleMapping>>>()
            }
            RoleMappingKindRaw::BingoRank => {
                let mut statement =
                    conn.prepare("SELECT role, rank FROM role_bingo_rank_config")?;
                statement
                    .query_map([], |row| {
                        Ok(RoleMapping::new(
                            RoleMappingKind::BingoRank {
                                rank: row.get("rank")?,
                            },
                            RoleId::new(row.get("role")?),
                        ))
                    })?
                    .collect::<Result<Vec<RoleMapping>>>()
            }
            RoleMappingKindRaw::NetworkBingo => {
                let mut statement =
                    conn.prepare("SELECT role, id FROM role_network_bingo_config")?;
                statement
                    .query_map([], |row| {
                        Ok(RoleMapping::new(
                            RoleMappingKind::NetworkBingo {
                                bingo: NetworkBingo::from_u8(row.get("id")?),
                            },
                            RoleId::new(row.get("role")?),
                        ))
                    })?
                    .collect::<Result<Vec<RoleMapping>>>()
            }
            RoleMappingKindRaw::Immortal => {
                let role_id: Option<u64> = conn
                    .query_one(
                        "SELECT immortal_role FROM role_config_global WHERE id=1",
                        [],
                        |row| row.get("immortal_role"),
                    )
                    .optional()?
                    .flatten();

                Ok(role_id.map_or_else(Vec::new, |id| {
                    vec![RoleMapping::new(RoleMappingKind::Immortal, RoleId::new(id))]
                }))
            }
        }
    }
}

pub struct GetRoleMapping {
    pub kind: RoleMappingKind,
}
impl DbRequest for GetRoleMapping {
    type ReturnValue = Result<Option<RoleMapping>>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        match self.kind {
            RoleMappingKind::Completions { count } => conn
                .query_one(
                    "SELECT role FROM role_completions_config WHERE count=?1",
                    params![count],
                    |row| Ok(RoleId::new(row.get("role")?)),
                )
                .optional(),
            RoleMappingKind::SpecificCompletion { bingo } => conn
                .query_one(
                    "SELECT role FROM role_specific_completion_config
WHERE bingo_kind=?1 AND kind_specific_id=?2",
                    params![bingo.kind as u8, bingo.kind_specific_id],
                    |row| Ok(RoleId::new(row.get("role")?)),
                )
                .optional(),
            RoleMappingKind::BingoRank { rank } => conn
                .query_one(
                    "SELECT role FROM role_bingo_rank_config WHERE rank=?1",
                    params![rank],
                    |row| Ok(RoleId::new(row.get("role")?)),
                )
                .optional(),
            RoleMappingKind::NetworkBingo { bingo } => conn
                .query_one(
                    "SELECT role FROM role_network_bingo_config WHERE id=?1",
                    [bingo as u8],
                    |row| Ok(RoleId::new(row.get("role")?)),
                )
                .optional(),
            RoleMappingKind::Immortal => conn
                .query_one(
                    "SELECT immortal_role FROM role_config_global WHERE id=1",
                    [],
                    |row| Ok(row.get("immortal_role").ok().map(RoleId::new)),
                )
                .optional()
                .map(Option::flatten),
        }
        .map(|opt| opt.map(|role| RoleMapping::new(self.kind, role)))
    }
}

pub struct BuildRoleDeltaCompletions {
    pub bingos: Vec<Bingo>,
    pub user_roles: Arc<FixedArray<RoleId>>,
}
impl DbRequest for BuildRoleDeltaCompletions {
    type ReturnValue = Result<RoleDelta>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let completion_count = self.bingos.len();

        let completions_template = GetRolePatterns.execute(conn)?.completions;

        let completions_role = GetRoleMapping {
            kind: RoleMappingKind::Completions {
                count: completion_count,
            },
        }
        .execute(conn)?;

        let completions_role = match completions_role {
            Some(role_mapping) => Some(BingoRole::Id(role_mapping.role)),
            None => completions_template.map(|t| BingoRole::Name {
                name: complete_completion_template(&t, completion_count),
                kind: RoleMappingKind::Completions {
                    count: completion_count,
                },
            }),
        };

        let specific_template = GetRolePatterns.execute(conn)?.specific_completion;

        let roles = self
            .bingos
            .into_iter()
            .filter(|b| b.kind != BingoKind::Normal || b.kind_specific_id == 0)
            .map(|b| {
                let role = GetRoleMapping {
                    kind: RoleMappingKind::SpecificCompletion { bingo: b },
                }
                .execute(conn)?;

                let bingo_role = match role {
                    Some(role_mapping) => Some(BingoRole::Id(role_mapping.role)),
                    None => specific_template.as_ref().map(|t| BingoRole::Name {
                        name: complete_bingo_template(t, &b),
                        kind: RoleMappingKind::SpecificCompletion { bingo: b },
                    }),
                };

                Ok(bingo_role)
            })
            .collect::<Result<Vec<_>>>()?;

        // drop all roles, for which there is neither an ID nor a template
        let mut roles: Vec<BingoRole> = roles.into_iter().flatten().collect();

        if let Some(role) = completions_role {
            roles.push(role);
        }

        let mut statement = conn.prepare(
            "
            SELECT role FROM role_completions_config
            UNION ALL
            SELECT role FROM role_specific_completion_config
            ",
        )?;

        let known_roles = statement
            .query_map([], |row| Ok(RoleId::new(row.get("role")?)))?
            .collect::<Result<Vec<_>>>()?;

        generate_role_delta(known_roles, self.user_roles, roles)
    }
}

pub struct BuildRoleDeltaBingoRank {
    pub rank: u8,
    pub user_roles: Arc<FixedArray<RoleId>>,
}
impl DbRequest for BuildRoleDeltaBingoRank {
    type ReturnValue = Result<RoleDelta>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let role = GetRoleMapping {
            kind: RoleMappingKind::BingoRank { rank: self.rank },
        }
        .execute(conn)?;

        let roles = match role {
            Some(role_mapping) => vec![BingoRole::Id(role_mapping.role)],
            None => {
                let template = GetRolePatterns.execute(conn)?.bingo_rank;

                template.map_or_else(Vec::new, |t| {
                    vec![BingoRole::Name {
                        name: complete_bingo_rank_template(&t, self.rank),
                        kind: RoleMappingKind::BingoRank { rank: self.rank },
                    }]
                })
            }
        };

        let mut statement = conn.prepare("SELECT role FROM role_bingo_rank_config")?;

        let known_roles = statement
            .query_map([], |row| Ok(RoleId::new(row.get("role")?)))?
            .collect::<Result<Vec<_>>>()?;

        generate_role_delta(known_roles, self.user_roles, roles)
    }
}

pub struct BuildRoleDeltaImmortal {
    pub user_roles: Arc<FixedArray<RoleId>>,
    pub has_achieved: bool,
}
impl DbRequest for BuildRoleDeltaImmortal {
    type ReturnValue = Result<RoleDelta>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let role = GetRoleMapping {
            kind: RoleMappingKind::Immortal,
        }
        .execute(conn)?;

        if let Some(role_mapping) = role
            && self.has_achieved
            && !self.user_roles.contains(&role_mapping.role)
        {
            Ok(RoleDelta {
                add: vec![BingoRole::Id(role_mapping.role)],
                remove: Vec::new(),
            })
        } else {
            Ok(RoleDelta {
                add: Vec::new(),
                remove: Vec::new(),
            })
        }
    }
}

pub struct BuildRoleDeltaNetworkBingos {
    pub bingos: Vec<NetworkBingo>,
    pub user_roles: Arc<FixedArray<RoleId>>,
}
impl DbRequest for BuildRoleDeltaNetworkBingos {
    type ReturnValue = Result<RoleDelta>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let roles: Vec<RoleMapping> = self
            .bingos
            .iter()
            .filter_map(|n| {
                GetRoleMapping {
                    kind: RoleMappingKind::NetworkBingo { bingo: *n },
                }
                .execute(conn)
                .transpose()
            })
            .collect::<Result<_>>()?;

        let roles = roles.into_iter().map(|r| BingoRole::Id(r.role)).collect();

        let mut statement = conn.prepare("SELECT role FROM role_network_bingo_config")?;

        let known_roles = statement
            .query_map([], |row| Ok(RoleId::new(row.get("role")?)))?
            .collect::<Result<Vec<RoleId>>>()?;

        generate_role_delta(known_roles, self.user_roles, roles)
    }
}

fn generate_role_delta(
    known_roles: Vec<RoleId>,
    user_has: Arc<FixedArray<RoleId>>,
    add: Vec<BingoRole>,
) -> Result<RoleDelta> {
    let add_ids: Vec<&RoleId> = add
        .iter()
        .filter_map(|role| match role {
            BingoRole::Id(id) => Some(id),
            _ => None,
        })
        .collect();

    let remove = known_roles
        .into_iter()
        .filter(|role| user_has.contains(role) && !add_ids.contains(&role))
        .collect();

    let add_filtered: Vec<BingoRole> = add
        .into_iter()
        .filter(|role| {
            !matches!(role,
                BingoRole::Id(id) if user_has.contains(id),
            )
        })
        .collect();

    Ok(RoleDelta {
        add: add_filtered,
        remove,
    })
}

pub struct GetRolePatterns;
impl DbRequest for GetRolePatterns {
    type ReturnValue = Result<RolePatterns>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let patterns = conn
        .query_one(
            "
            SELECT completion_pattern, special_completion_pattern, bingo_rank_pattern, immortal_pattern
            FROM role_config_global WHERE id=1
            ",
            [],
            |row| {
                Ok(RolePatterns {
                    completions: row.get("completion_pattern")?,
                    specific_completion: row.get("special_completion_pattern")?,
                    bingo_rank: row.get("bingo_rank_pattern")?,
                    immortal: row.get("immortal_pattern")?,
                })
            },
        )
        .optional()?;

        Ok(patterns.unwrap_or_default())
    }
}

pub fn complete_completion_template(template: &str, count: usize) -> String {
    template.replace("{count}", &count.to_string())
}

pub fn complete_bingo_template(template: &str, bingo: &Bingo) -> String {
    template
        .replace("{number}", &(bingo.kind_specific_id + 1).to_string())
        .replace("{kind}", bingo.kind.as_prefix())
}

pub fn complete_bingo_rank_template(template: &str, rank: u8) -> String {
    template.replace("{rank}", &rank.to_string())
}
