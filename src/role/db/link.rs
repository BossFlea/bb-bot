use poise::serenity_prelude::UserId;
use rusqlite::{Connection, OptionalExtension as _, Result, params};

use crate::db::DbRequest;
use crate::role::types::LinkedUser;

pub struct GetLinkedUserByDiscord {
    pub discord: UserId,
}
impl DbRequest for GetLinkedUserByDiscord {
    type ReturnValue = Result<Option<LinkedUser>>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        conn.query_one(
            "SELECT minecraft_uuid FROM role_users_linked WHERE discord_id=?1",
            params![self.discord.get()],
            |row| row.get("minecraft_uuid"),
        )
        .map(|uuid| LinkedUser::new(self.discord, uuid))
        .optional()
    }
}

pub struct GetLinkedUserByMinecraft {
    pub mc_uuid: String,
}
impl DbRequest for GetLinkedUserByMinecraft {
    type ReturnValue = Result<Option<LinkedUser>>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        conn.query_one(
            "SELECT discord_id FROM role_users_linked WHERE minecraft_uuid=?1",
            params![self.mc_uuid],
            |row| row.get("discord_id"),
        )
        .map(|id| LinkedUser::new(UserId::new(id), self.mc_uuid))
        .optional()
    }
}

pub struct InsertLinkedUser {
    pub user: LinkedUser,
}
impl DbRequest for InsertLinkedUser {
    /// duplicate discord/minecraft
    type ReturnValue = Result<(Option<UserId>, Option<String>)>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let transaction = conn.transaction()?;

        let duplicate_uuid: Option<String> = {
            let mut statement = transaction
                .prepare("SELECT minecraft_uuid FROM role_users_linked WHERE discord_id=?1")?;

            statement
                .query_one(params![self.user.discord.get()], |row| {
                    row.get("minecraft_uuid")
                })
                .optional()?
        };
        if duplicate_uuid.is_some() {
            return Ok((None, duplicate_uuid));
        }

        let duplicate_discord: Option<u64> = {
            let mut statement = transaction
                .prepare("SELECT discord_id FROM role_users_linked WHERE minecraft_uuid=?1")?;

            statement
                .query_one(params![self.user.mc_uuid], |row| row.get("discord_id"))
                .optional()?
        };
        if duplicate_discord.is_some() {
            return Ok((duplicate_discord.map(UserId::new), None));
        }

        transaction.execute(
            "
            INSERT INTO role_users_linked (discord_id, minecraft_uuid)
            VALUES (?1, ?2)
            ",
            params![self.user.discord.get(), self.user.mc_uuid],
        )?;

        transaction.commit()?;

        Ok((None, None))
    }
}

pub struct UpdateLinkedUser {
    pub user: LinkedUser,
}
impl DbRequest for UpdateLinkedUser {
    type ReturnValue = Result<()>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let transaction = conn.transaction()?;

        transaction.execute(
            "
            DELETE FROM role_users_linked
            WHERE discord_id=?1 OR minecraft_uuid=?2
            ",
            params![self.user.discord.get(), self.user.mc_uuid],
        )?;

        transaction.execute(
            "
            INSERT INTO role_users_linked (discord_id, minecraft_uuid)
            VALUES (?1, ?2)
            ",
            params![self.user.discord.get(), self.user.mc_uuid],
        )?;

        transaction.commit()?;

        Ok(())
    }
}

pub struct RemoveLinkedUserByDiscord {
    pub discord: UserId,
}
impl DbRequest for RemoveLinkedUserByDiscord {
    type ReturnValue = Result<Option<LinkedUser>>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let mut statement = conn.prepare(
            "
            DELETE FROM role_users_linked
            WHERE discord_id=?1
            RETURNING minecraft_uuid
            ",
        )?;

        statement
            .query_one(params![self.discord.get()], |row| row.get("minecraft_uuid"))
            .map(|uuid| LinkedUser::new(self.discord, uuid))
            .optional()
    }
}

pub struct RemoveLinkedUserByMinecraft {
    pub mc_uuid: String,
}
impl DbRequest for RemoveLinkedUserByMinecraft {
    type ReturnValue = Result<Option<LinkedUser>>;

    fn execute(self, conn: &mut Connection) -> Self::ReturnValue {
        let mut statement = conn.prepare(
            "
            DELETE FROM role_users_linked
            WHERE minecraft_uuid=?1
            RETURNING discord_id
            ",
        )?;

        statement
            .query_one(params![self.mc_uuid], |row| row.get("discord_id"))
            .map(|id| LinkedUser::new(UserId::new(id), self.mc_uuid))
            .optional()
    }
}
