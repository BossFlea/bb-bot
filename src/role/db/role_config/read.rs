use std::sync::Arc;

use poise::serenity_prelude::{RoleId, small_fixed_array::FixedArray};
use rusqlite::{Connection, Error, OptionalExtension as _, params};

use crate::role::types::{
    BingoRole, NetworkBingo, RoleDelta, RoleMapping, RoleMappingKind, RoleMappingKindRaw,
    RolePatterns,
};
use crate::shared::types::{Bingo, BingoKind};

pub fn mappings_by_kind(
    conn: &Connection,
    kind: &RoleMappingKindRaw,
) -> Result<Vec<RoleMapping>, Error> {
    match kind {
        RoleMappingKindRaw::Completions => {
            let mut statement = conn.prepare("SELECT role, count FROM role_completions_config")?;
            statement
                .query_map([], |row| {
                    Ok(RoleMapping {
                        kind: RoleMappingKind::Completions {
                            count: row.get("count")?,
                        },
                        role: RoleId::new(row.get("role")?),
                    })
                })?
                .collect::<Result<Vec<RoleMapping>, Error>>()
        }
        RoleMappingKindRaw::SpecificCompletion => {
            let mut statement = conn.prepare(
                "SELECT role, kind_specific_id, bingo_kind FROM role_specific_completion_config",
            )?;
            statement
                .query_map([], |row| {
                    Ok(RoleMapping {
                        kind: RoleMappingKind::SpecificCompletion {
                            bingo: Bingo::new(
                                row.get("kind_specific_id")?,
                                BingoKind::from_u8(row.get("bingo_kind")?),
                                None,
                            ),
                        },
                        role: RoleId::new(row.get("role")?),
                    })
                })?
                .collect::<Result<Vec<RoleMapping>, Error>>()
        }
        RoleMappingKindRaw::BingoRank => {
            let mut statement = conn.prepare("SELECT role, rank FROM role_bingo_rank_config")?;
            statement
                .query_map([], |row| {
                    Ok(RoleMapping {
                        kind: RoleMappingKind::BingoRank {
                            rank: row.get("rank")?,
                        },
                        role: RoleId::new(row.get("role")?),
                    })
                })?
                .collect::<Result<Vec<RoleMapping>, Error>>()
        }
        RoleMappingKindRaw::NetworkBingo => {
            let mut statement = conn.prepare("SELECT role, id FROM role_network_bingo_config")?;
            statement
                .query_map([], |row| {
                    Ok(RoleMapping {
                        kind: RoleMappingKind::NetworkBingo {
                            bingo: NetworkBingo::from_u8(row.get("id")?),
                        },
                        role: RoleId::new(row.get("role")?),
                    })
                })?
                .collect::<Result<Vec<RoleMapping>, Error>>()
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
                vec![RoleMapping {
                    kind: RoleMappingKind::Immortal,
                    role: RoleId::new(id),
                }]
            }))
        }
    }
}

pub fn role(conn: &Connection, kind: &RoleMappingKind) -> Result<Option<RoleId>, Error> {
    match kind {
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
                [*bingo as u8],
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
}

pub fn roles_from_bingos(
    conn: &Connection,
    bingos: Vec<Bingo>,
    user_roles: Arc<FixedArray<RoleId>>,
) -> Result<RoleDelta, Error> {
    let completions = bingos.len();

    let completions_template: Option<String> = conn
        .query_one(
            "SELECT completion_pattern FROM role_config_global WHERE id=1",
            [],
            |row| row.get("completion_pattern"),
        )
        .optional()?;

    let completions_role_id = conn
        .query_one(
            "
            SELECT role FROM role_completions_config
            WHERE count=?1
            ",
            params![completions],
            |row| Ok(RoleId::new(row.get("role")?)),
        )
        .optional()?;

    let completions_role = match completions_role_id {
        Some(id) => Some(BingoRole::Id(id)),
        None => completions_template.map(|t| BingoRole::Name {
            name: complete_completion_template(&t, completions),
            kind: RoleMappingKind::Completions { count: completions },
        }),
    };

    let specific_template: Option<String> = conn
        .query_one(
            "SELECT special_completion_pattern FROM role_config_global WHERE id=1",
            [],
            |row| row.get("special_completion_pattern"),
        )
        .optional()?;

    let mut specific_statement = conn.prepare(
        "
        SELECT role FROM role_specific_completion_config
        WHERE bingo_kind=?1 AND kind_specific_id=?2
        ",
    )?;

    let roles = bingos
        .into_iter()
        .filter(|b| b.kind != BingoKind::Normal || b.kind_specific_id == 0)
        .map(|b| {
            let role = specific_statement
                .query_one(params![b.kind as u8, b.kind_specific_id], |row| {
                    Ok(RoleId::new(row.get("role")?))
                })
                .optional()?;

            let bingo_role = match role {
                Some(id) => Some(BingoRole::Id(id)),
                None => specific_template.as_ref().map(|t| BingoRole::Name {
                    name: complete_bingo_template(t, &b),
                    kind: RoleMappingKind::SpecificCompletion { bingo: b },
                }),
            };

            Ok(bingo_role)
        })
        .collect::<Result<Vec<Option<BingoRole>>, Error>>()?;

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
        .collect::<Result<Vec<RoleId>, Error>>()?;

    generate_role_delta(known_roles, user_roles, roles)
}

pub fn role_bingo_rank(
    conn: &Connection,
    rank: u8,
    user_roles: Arc<FixedArray<RoleId>>,
) -> Result<RoleDelta, Error> {
    let role_id = conn
        .query_one(
            "SELECT role FROM role_bingo_rank_config WHERE rank=?1",
            params![rank],
            |row| Ok(RoleId::new(row.get("role")?)),
        )
        .optional()?;

    let roles = match role_id {
        Some(id) => vec![BingoRole::Id(id)],
        None => {
            let template: Option<String> = conn
                .query_one(
                    "SELECT bingo_rank_pattern FROM role_config_global WHERE id=1",
                    [],
                    |row| row.get("bingo_rank_pattern"),
                )
                .optional()?;

            template.map_or_else(Vec::new, |t| {
                vec![BingoRole::Name {
                    name: complete_bingo_rank_template(&t, rank),
                    kind: RoleMappingKind::BingoRank { rank },
                }]
            })
        }
    };

    let mut statement = conn.prepare("SELECT role FROM role_bingo_rank_config")?;

    let known_roles = statement
        .query_map([], |row| Ok(RoleId::new(row.get("role")?)))?
        .collect::<Result<Vec<RoleId>, Error>>()?;

    generate_role_delta(known_roles, user_roles, roles)
}

pub fn role_immortal(
    conn: &Connection,
    user_roles: Arc<FixedArray<RoleId>>,
) -> Result<Option<BingoRole>, Error> {
    let role_id = conn
        .query_one(
            "SELECT immortal_role FROM role_config_global WHERE id=1",
            [],
            |row| Ok(RoleId::new(row.get("immortal_role")?)),
        )
        .optional()?;

    if let Some(id) = role_id
        && !user_roles.contains(&id)
    {
        Ok(Some(BingoRole::Id(id)))
    } else {
        Ok(None)
    }
}

pub fn roles_from_network_bingos(
    conn: &Connection,
    bingos: &[NetworkBingo],
    user_roles: Arc<FixedArray<RoleId>>,
) -> Result<RoleDelta, Error> {
    let mut statement = conn.prepare(
        "
        SELECT role FROM role_network_bingo_config
        WHERE id=?1
        ",
    )?;

    let role_ids: Vec<RoleId> = bingos
        .iter()
        .filter_map(|n| {
            statement
                .query_one(params![*n as u8], |row| Ok(RoleId::new(row.get("role")?)))
                .optional()
                .transpose()
        })
        .collect::<Result<_, Error>>()?;

    let roles = role_ids.into_iter().map(BingoRole::Id).collect();

    let mut statement = conn.prepare("SELECT role FROM role_network_bingo_config")?;

    let known_roles = statement
        .query_map([], |row| Ok(RoleId::new(row.get("role")?)))?
        .collect::<Result<Vec<RoleId>, Error>>()?;

    generate_role_delta(known_roles, user_roles, roles)
}

fn generate_role_delta(
    known_roles: Vec<RoleId>,
    user_has: Arc<FixedArray<RoleId>>,
    add: Vec<BingoRole>,
) -> Result<RoleDelta, Error> {
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

pub fn get_role_patterns(conn: &Connection) -> Result<RolePatterns, Error> {
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

    Ok(patterns.unwrap_or(RolePatterns {
        completions: None,
        specific_completion: None,
        bingo_rank: None,
        immortal: None,
    }))
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
