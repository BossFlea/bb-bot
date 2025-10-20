use poise::serenity_prelude::{Role, RoleId};
use rusqlite::{Connection, Result, params};

use crate::db::DbRequest;
use crate::role::{
    db::role_config::read,
    types::{RoleMapping, RoleMappingKind, RolePatterns},
};
use crate::shared::types::{Bingo, BingoKind};

pub struct InsertRoleMapping {
    pub role_mapping: RoleMapping,
}
impl DbRequest for InsertRoleMapping {
    type ReturnValue = Result<()>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        insert_role_mapping(conn, &self.role_mapping)
    }
}

fn insert_role_mapping(conn: &Connection, role_mapping: &RoleMapping) -> Result<()> {
    match role_mapping.kind {
        RoleMappingKind::Completions { count } => conn.execute(
            "
            INSERT OR REPLACE INTO role_completions_config (role, count)
            VALUES (?1, ?2)
            ",
            params![role_mapping.role.get(), count],
        ),
        RoleMappingKind::SpecificCompletion { bingo } => conn.execute(
            "
            INSERT OR REPLACE INTO role_specific_completion_config
            (role, kind_specific_id, bingo_kind)
            VALUES (?1, ?2, ?3)
            ",
            params![
                role_mapping.role.get(),
                bingo.kind_specific_id,
                bingo.kind as u8
            ],
        ),
        RoleMappingKind::BingoRank { rank } => conn.execute(
            "
            INSERT OR REPLACE INTO role_bingo_rank_config (role, rank)
            VALUES (?1, ?2)
            ",
            params![role_mapping.role.get(), rank],
        ),
        RoleMappingKind::Immortal => conn.execute(
            "
            INSERT INTO role_config_global (id, immortal_role)
            VALUES (1, ?1)
            ON CONFLICT(id) DO UPDATE SET
                immortal_role = excluded.immortal_role
            ",
            params![role_mapping.role.get()],
        ),
        RoleMappingKind::NetworkBingo { bingo } => conn.execute(
            "
            INSERT OR REPLACE INTO role_network_bingo_config (role, id)
            VALUES (?1, ?2)
            ",
            params![role_mapping.role.get(), bingo as u8],
        ),
    }?;
    Ok(())
}

pub struct DeleteRoleMappingByRole {
    pub role: RoleId,
}
impl DbRequest for DeleteRoleMappingByRole {
    type ReturnValue = Result<()>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let transaction = conn.transaction()?;

        for table in [
            "role_completions_config",
            "role_specific_completion_config",
            "role_bingo_rank_config",
            "role_network_bingo_config",
        ] {
            transaction.execute(
                &format!("DELETE FROM {table} WHERE role=?1"),
                params![self.role.get()],
            )?;
        }

        transaction.execute(
            "
            UPDATE role_config_global
            SET immortal_role=NULL
            WHERE id=1 AND immortal_role=?1
            ",
            params![self.role.get()],
        )?;

        transaction.commit()
    }
}

pub struct DetectRelevantRoles {
    pub roles: Vec<Role>,
}
impl DbRequest for DetectRelevantRoles {
    type ReturnValue = Result<Vec<RoleMapping>>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let patterns = read::GetRolePatterns.execute(conn)?;

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
                    &self.roles,
                );

                if let Some(id) = role {
                    let role_mapping = RoleMapping {
                        kind: RoleMappingKind::Completions { count },
                        role: id,
                    };
                    insert_role_mapping(conn, &role_mapping)?;
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
                        &self.roles,
                    );

                    if let Some(id) = role {
                        let role_mapping = RoleMapping {
                            kind: RoleMappingKind::SpecificCompletion { bingo },
                            role: id,
                        };
                        insert_role_mapping(conn, &role_mapping)?;
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
                    &self.roles,
                );

                if let Some(id) = role {
                    let role_mapping = RoleMapping {
                        kind: RoleMappingKind::BingoRank { rank },
                        role: id,
                    };
                    insert_role_mapping(conn, &role_mapping)?;
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
                let role = find_pattern_role(&immortal_template, &self.roles);
                if let Some(id) = role {
                    let role_mapping = RoleMapping {
                        kind: RoleMappingKind::Immortal,
                        role: id,
                    };
                    insert_role_mapping(conn, &role_mapping)?;
                    detected_roles.push(role_mapping);
                }
            }
        }

        Ok(detected_roles)
    }
}

fn find_pattern_role(pattern: &str, roles: &[Role]) -> Option<RoleId> {
    roles
        .iter()
        .find_map(|r| (r.name == pattern).then_some(r.id))
}

pub struct SetRolePatterns {
    pub patterns: RolePatterns,
}
impl DbRequest for SetRolePatterns {
    type ReturnValue = Result<()>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        conn.execute(
            "
            INSERT INTO role_config_global
            (id, completion_pattern, special_completion_pattern,
                bingo_rank_pattern, immortal_pattern)
            VALUES (1, ?1, ?2, ?3, ?4)
            ON CONFLICT(id) DO UPDATE SET
                completion_pattern = excluded.completion_pattern,
                special_completion_pattern = excluded.special_completion_pattern,
                bingo_rank_pattern = excluded.bingo_rank_pattern,
                immortal_pattern = excluded.immortal_pattern
            ",
            params![
                self.patterns.completions,
                self.patterns.specific_completion,
                self.patterns.bingo_rank,
                self.patterns.immortal,
            ],
        )?;
        Ok(())
    }
}
