use rusqlite::{Connection, OptionalExtension as _, Result, Statement, ToSql, params};

use crate::db::DbRequest;
use crate::hob::types::{HobEntry, OneOffPlayers, OngoingSubentry};
use crate::shared::types::{Bingo, BingoKind};

struct PartialHobEntryOneOff {
    id: u64,
    title: String,
    comment: Option<String>,
    bingo: Bingo,
}
struct PartialHobEntryOngoing {
    id: u64,
    title: String,
    comment: Option<String>,
}

pub struct GetAllHobEntries;
impl DbRequest for GetAllHobEntries {
    type ReturnValue = Result<Vec<HobEntry>>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let oneoff_statement = conn.prepare(
            "
            SELECT e.id, e.title, e.comment, e.bingo, e.bingo_kind, m.bingo AS sort_value
            FROM hob_entries_oneoff e
            LEFT JOIN bingo_kind_id_map m
                ON e.bingo_kind = m.bingo_kind
                AND e.bingo = m.kind_specific_id
            ORDER BY COALESCE(m.bingo, e.bingo) DESC;
            ",
        )?;
        let ongoing_statement = conn.prepare(
            "
            SELECT e.id, e.title, e.comment
            FROM hob_entries_ongoing e
            LEFT JOIN (
                SELECT entry_id, MAX(COALESCE(m.bingo, s.bingo)) AS sort_value
                FROM hob_ongoing_subentries s
                LEFT JOIN bingo_kind_id_map m
                    ON s.bingo_kind = m.bingo_kind
                    AND s.bingo = m.kind_specific_id
                GROUP BY s.entry_id
            ) s_max ON e.id = s_max.entry_id
            ORDER BY s_max.sort_value DESC;
            ",
        )?;

        query_entries(conn, oneoff_statement, ongoing_statement, &[])
    }
}

pub struct GetHobEntry {
    pub id: u64,
}
impl DbRequest for GetHobEntry {
    type ReturnValue = Result<Option<HobEntry>>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let oneoff_statement = conn.prepare(
            "
            SELECT e.id, e.title, e.comment, e.bingo, e.bingo_kind, m.bingo AS sort_value
            FROM hob_entries_oneoff e
            LEFT JOIN bingo_kind_id_map m
                ON e.bingo_kind = m.bingo_kind
                AND e.bingo = m.kind_specific_id
            WHERE id=?1
            ",
        )?;
        let ongoing_statement = conn.prepare(
            "
            SELECT id, title, comment
            FROM hob_entries_ongoing
            WHERE id=?1
            ",
        )?;

        query_entries(conn, oneoff_statement, ongoing_statement, params![self.id])
            .map(|vec| vec.into_iter().next())
    }
}

pub struct GetHobSubentry {
    pub id: u64,
    pub entry_id: u64,
}
impl DbRequest for GetHobSubentry {
    type ReturnValue = Result<Option<OngoingSubentry>>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let mut statement = conn.prepare(
            "
            SELECT s.player, s.value, s.bingo, s.bingo_kind, m.bingo AS sort_value
            FROM hob_ongoing_subentries s
            LEFT JOIN bingo_kind_id_map m
                ON s.bingo_kind = m.bingo_kind
                AND s.bingo = m.kind_specific_id
            WHERE id=?1 AND entry_id=?2
            ",
        )?;
        statement
            .query_row([self.id, self.entry_id], |row| {
                Ok(OngoingSubentry {
                    id: self.id,
                    entry_id: self.entry_id,
                    player: row.get("player")?,
                    value: row.get("value")?,
                    bingo: Bingo {
                        kind_specific_id: row.get("bingo")?,
                        kind: BingoKind::from_u8(row.get("bingo_kind")?),
                        unique_id: row.get("sort_value")?,
                    },
                })
            })
            .optional()
    }
}

pub struct SearchEntriesContent {
    pub query: String,
}
impl DbRequest for SearchEntriesContent {
    type ReturnValue = Result<Vec<HobEntry>>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let oneoff_statement = conn.prepare(
            "
            SELECT DISTINCT e.id, e.title, e.comment, e.bingo, e.bingo_kind, m.bingo AS sort_value
            FROM hob_entries_oneoff e
            LEFT JOIN hob_oneoff_players p ON e.id = p.entry_id
            LEFT JOIN bingo_kind_id_map m
                ON e.bingo_kind = m.bingo_kind
                AND e.bingo = m.kind_specific_id
            WHERE e.title LIKE '%' || ?1 || '%' ESCAPE '\\'
                OR e.comment LIKE '%' || ?1 || '%' ESCAPE '\\'
                OR p.player LIKE '%' || ?1 || '%' ESCAPE '\\'
                OR (
                    CASE e.bingo_kind
                        WHEN 0 THEN 'normal bingo #'
                        WHEN 1 THEN 'extreme bingo #'
                        WHEN 2 THEN 'secret bingo #'
                        ELSE 'unknown bingo #'
                    END
                    || CAST(e.bingo AS TEXT)
                ) LIKE '%' || ?1 || '%' ESCAPE '\\'
            ORDER BY COALESCE(m.bingo, e.bingo) DESC;
            ",
        )?;
        let ongoing_statement = conn.prepare(
            "
            SELECT e.id, e.title, e.comment
            FROM hob_entries_ongoing e
            LEFT JOIN (
                SELECT entry_id, MAX(COALESCE(m.bingo, s.bingo)) AS sort_value
                FROM hob_ongoing_subentries s
                LEFT JOIN bingo_kind_id_map m
                    ON s.bingo_kind = m.bingo_kind
                    AND s.bingo = m.kind_specific_id
                GROUP BY s.entry_id
            ) s_max ON e.id = s_max.entry_id
            WHERE e.title LIKE '%' || ?1 || '%' ESCAPE '\\'
                OR e.comment LIKE '%' || ?1 || '%' ESCAPE '\\'
                OR EXISTS (
                    SELECT 1
                    FROM hob_ongoing_subentries s
                    WHERE s.entry_id = e.id
                    AND (s.player LIKE '%' || ?1 || '%' ESCAPE '\\'
                        OR s.value LIKE '%' || ?1 || '%' ESCAPE '\\'
                        OR (
                            CASE s.bingo_kind
                                WHEN 0 THEN 'normal bingo #'
                                WHEN 1 THEN 'extreme bingo #'
                                WHEN 2 THEN 'secret bingo #'
                                ELSE 'bingo #'
                            END
                            || CAST(s.bingo AS TEXT)
                        ) LIKE '%' || ?1 || '%' ESCAPE '\\'
                    )
                )
            ORDER BY s_max.sort_value DESC;
            ",
        )?;

        query_entries(
            conn,
            oneoff_statement,
            ongoing_statement,
            params![self.query],
        )
    }
}

fn query_entries(
    conn: &Connection,
    mut oneoff_statement: Statement,
    mut ongoing_statement: Statement,
    params: &[&dyn ToSql],
) -> Result<Vec<HobEntry>> {
    let oneoff_entries = {
        let partial_entries_oneoff = {
            oneoff_statement
                .query_map(params, |row| {
                    Ok(PartialHobEntryOneOff {
                        id: row.get("id")?,
                        title: row.get("title")?,
                        comment: row.get("comment")?,
                        bingo: Bingo {
                            kind_specific_id: row.get("bingo")?,
                            kind: BingoKind::from_u8(row.get("bingo_kind")?),
                            unique_id: row.get("sort_value")?,
                        },
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        partial_entries_oneoff
            .into_iter()
            .map(|partial_data| {
                let players = get_oneoff_players(conn, partial_data.id)?;
                Ok(HobEntry::OneOff {
                    id: partial_data.id,
                    title: partial_data.title,
                    comment: partial_data.comment,
                    bingo: partial_data.bingo,
                    players: OneOffPlayers { players },
                })
            })
            .collect::<Result<Vec<_>>>()?
    };

    let ongoing_entries = {
        let partial_entries_ongoing = {
            ongoing_statement
                .query_map(params, |row| {
                    Ok(PartialHobEntryOngoing {
                        id: row.get("id")?,
                        title: row.get("title")?,
                        comment: row.get("comment")?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?
        };

        partial_entries_ongoing
            .into_iter()
            .map(|partial_data| {
                let subentries = get_ongoing_subentries(conn, partial_data.id)?;
                Ok(HobEntry::Ongoing {
                    id: partial_data.id,
                    title: partial_data.title,
                    comment: partial_data.comment,
                    subentries,
                })
            })
            .collect::<Result<Vec<_>>>()?
    };

    Ok(merge_sorted_entry_vecs(oneoff_entries, ongoing_entries))
}

fn get_oneoff_players(conn: &Connection, entry_id: u64) -> Result<Vec<String>> {
    let mut statement = conn
        .prepare("SELECT player FROM hob_oneoff_players WHERE entry_id=?1 ORDER BY position ASC")?;
    statement
        .query_map([entry_id], |row| row.get("player"))?
        .collect()
}

fn get_ongoing_subentries(conn: &Connection, entry_id: u64) -> Result<Vec<OngoingSubentry>> {
    let mut statement = conn.prepare(
        "
        SELECT s.id, s.entry_id, s.player, s.value, s.bingo, s.bingo_kind, m.bingo AS sort_value
        FROM hob_ongoing_subentries s
        LEFT JOIN bingo_kind_id_map m
            ON s.bingo_kind = m.bingo_kind
            AND s.bingo = m.kind_specific_id
        WHERE s.entry_id=?1
        ORDER BY COALESCE(m.bingo, s.bingo) DESC
        ",
    )?;
    statement
        .query_map([entry_id], |row| {
            Ok(OngoingSubentry {
                id: row.get("id")?,
                entry_id: row.get("entry_id")?,
                player: row.get("player")?,
                value: row.get("value")?,
                bingo: Bingo {
                    kind_specific_id: row.get("bingo")?,
                    kind: BingoKind::from_u8(row.get("bingo_kind")?),
                    unique_id: row.get("sort_value")?,
                },
            })
        })?
        .collect()
}

fn merge_sorted_entry_vecs(vec_a: Vec<HobEntry>, vec_b: Vec<HobEntry>) -> Vec<HobEntry> {
    let mut merged = Vec::with_capacity(vec_a.len() + vec_b.len());

    let mut iter_a = vec_a.into_iter().peekable();
    let mut iter_b = vec_b.into_iter().peekable();

    while let (Some(next_a), Some(next_b)) = (iter_a.peek(), iter_b.peek()) {
        // unwraps are safe because peek confirms existence of element
        if next_a.get_bingo_num() > next_b.get_bingo_num() {
            merged.push(iter_a.next().unwrap());
        } else {
            merged.push(iter_b.next().unwrap());
        }
    }

    merged.extend(iter_a);
    merged.extend(iter_b);
    merged
}
