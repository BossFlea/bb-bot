use poise::serenity_prelude::{Role, RoleId};
use rusqlite::{Connection, Error, params};

use crate::role::{
    db::role_config::read,
    types::{RoleMapping, RoleMappingKind, RolePatterns},
};
use crate::shared::types::{Bingo, BingoKind};

pub fn insert_role_mapping(
    conn: &Connection,
    role: &RoleId,
    kind: &RoleMappingKind,
) -> Result<(), Error> {
    match kind {
        RoleMappingKind::Completions { count } => conn.execute(
            "
            INSERT OR REPLACE INTO role_completions_config (role, count)
            VALUES (?1, ?2)
            ",
            params![role.get(), count],
        ),
        RoleMappingKind::SpecificCompletion { bingo } => conn.execute(
            "
            INSERT OR REPLACE INTO role_specific_completion_config (role, kind_specific_id, bingo_kind)
            VALUES (?1, ?2, ?3)
            ",
            params![role.get(), bingo.kind_specific_id, bingo.kind as u8],
        ),
        RoleMappingKind::BingoRank { rank } => conn.execute(
            "
            INSERT OR REPLACE INTO role_bingo_rank_config (role, rank)
            VALUES (?1, ?2)
            ",
            params![role.get(), rank],
        ),
        RoleMappingKind::Immortal => conn.execute(
            "
            INSERT INTO role_config_global (id, immortal_role)
            VALUES (1, ?1)
            ON CONFLICT(id) DO UPDATE SET
                immortal_role = excluded.immortal_role
            ",
            params![role.get()],
        ),
        RoleMappingKind::NetworkBingo { bingo } => conn.execute(
            "
            INSERT OR REPLACE INTO role_network_bingo_config (role, id)
            VALUES (?1, ?2)
            ",
            params![role.get(), *bingo as u8],
        ),
    }
    .map(|_| ())
}

pub fn delete_role_mapping_by_role(conn: &mut Connection, role: &RoleId) -> Result<(), Error> {
    let transaction = conn.transaction()?;

    for table in [
        "role_completions_config",
        "role_specific_completion_config",
        "role_bingo_rank_config",
        "role_network_bingo_config",
    ] {
        transaction.execute(
            &format!("DELETE FROM {table} WHERE role=?1"),
            params![role.get()],
        )?;
    }

    transaction.execute(
        "UPDATE role_config_global SET immortal_role=NULL WHERE id=1 AND immortal_role=?1",
        params![role.get()],
    )?;

    transaction.commit()
}

pub fn detect_roles(conn: &Connection, roles: &[Role]) -> Result<Vec<RoleMapping>, Error> {
    let patterns = read::get_role_patterns(conn)?;

    let mut detected_roles: Vec<RoleMapping> = Vec::new();

    if let Some(completion_template) = patterns.completions {
        let mut statement = conn.prepare(
            "
            SELECT 1 FROM role_completions_config
            WHERE count=?1
            ",
        )?;

        let mut not_found_contiguous = 0;
        let mut count = 1;
        while not_found_contiguous < 3 {
            if statement.exists(params![count])? {
                count += 1;
                continue;
            }

            let role = find_pattern_role(
                &read::complete_completion_template(&completion_template, count),
                roles,
            );

            if let Some(id) = role {
                let role_mapping = RoleMapping {
                    kind: RoleMappingKind::Completions { count },
                    role: id,
                };
                insert_role_mapping(conn, &role_mapping.role, &role_mapping.kind)?;
                detected_roles.push(role_mapping);
                not_found_contiguous = 0;
            } else {
                not_found_contiguous += 1;
            }
            count += 1;
        }
    }

    if let Some(specific_completion_template) = patterns.specific_completion {
        let mut statement = conn.prepare(
            "
            SELECT 1 FROM role_specific_completion_config
            WHERE kind_specific_id=?1 AND bingo_kind=?2
            ",
        )?;

        for &kind in BingoKind::ALL {
            let mut not_found_contiguous = 0;
            let mut bingo_id = 0;
            while not_found_contiguous < 3 {
                if statement.exists(params![bingo_id, kind as u8])? {
                    bingo_id += 1;
                    continue;
                }

                let bingo = Bingo::new(bingo_id, kind, None);
                let role = find_pattern_role(
                    &read::complete_bingo_template(&specific_completion_template, &bingo),
                    roles,
                );

                if let Some(id) = role {
                    let role_mapping = RoleMapping {
                        kind: RoleMappingKind::SpecificCompletion { bingo },
                        role: id,
                    };
                    insert_role_mapping(conn, &role_mapping.role, &role_mapping.kind)?;
                    detected_roles.push(role_mapping);
                    not_found_contiguous = 0;
                } else {
                    not_found_contiguous += 1;
                }
                bingo_id += 1;
            }
        }
    }

    if let Some(bingo_rank_template) = patterns.bingo_rank {
        let mut statement = conn.prepare(
            "
            SELECT 1 FROM role_bingo_rank_config
            WHERE rank=?1
            ",
        )?;

        let mut not_found_contiguous = 0;
        let mut rank = 1;
        while not_found_contiguous < 3 {
            if statement.exists(params![rank])? {
                rank += 1;
                continue;
            }

            let role = find_pattern_role(
                &read::complete_bingo_rank_template(&bingo_rank_template, rank),
                roles,
            );

            if let Some(id) = role {
                let role_mapping = RoleMapping {
                    kind: RoleMappingKind::BingoRank { rank },
                    role: id,
                };
                insert_role_mapping(conn, &id, &role_mapping.kind)?;
                detected_roles.push(role_mapping);
                not_found_contiguous = 0;
            } else {
                not_found_contiguous += 1;
            }
            rank += 1;
        }
    }

    if let Some(immortal_template) = patterns.immortal {
        let mut statement = conn.prepare(
            "
            SELECT 1 FROM role_config_global
            WHERE id=1 AND immortal_role NOT NULL
            ",
        )?;

        if !statement.exists([])? {
            let role = find_pattern_role(&immortal_template, roles);
            if let Some(id) = role {
                let role_mapping = RoleMapping {
                    kind: RoleMappingKind::Immortal,
                    role: id,
                };
                insert_role_mapping(conn, &role_mapping.role, &role_mapping.kind)?;
                detected_roles.push(role_mapping);
            }
        }
    }

    Ok(detected_roles)
}

fn find_pattern_role(pattern: &str, roles: &[Role]) -> Option<RoleId> {
    roles
        .iter()
        .find_map(|r| (r.name == pattern).then_some(r.id))
}

pub fn set_role_patterns(conn: &mut Connection, patterns: &RolePatterns) -> Result<(), Error> {
    conn.execute(
        "
            INSERT INTO role_config_global
            (id, completion_pattern, special_completion_pattern, bingo_rank_pattern, immortal_pattern)
            VALUES (1, ?1, ?2, ?3, ?4)
            ON CONFLICT(id) DO UPDATE SET
                completion_pattern = excluded.completion_pattern,
                special_completion_pattern = excluded.special_completion_pattern,
                bingo_rank_pattern = excluded.bingo_rank_pattern,
                immortal_pattern = excluded.immortal_pattern
            ",
        params![
            &patterns.completions,
            &patterns.specific_completion,
            &patterns.bingo_rank,
            &patterns.immortal,
        ],
    )
    .map(|_| ())
}
