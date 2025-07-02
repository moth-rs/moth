use ::serenity::{
    all::{GenericChannelId, MessageId, ReactionType, RoleId},
    futures::FutureExt,
    small_fixed_array::{self, FixedString},
};
use chrono::Utc;
use dashmap::{DashMap, DashSet};
use parking_lot::Mutex;
use regex::Regex;
use rosu_v2::{model::GameMode, prelude::UserExtended};
use serenity::all::UserId;
use sqlx::{Executor, PgPool, postgres::PgPoolOptions, query};
use std::{
    collections::{HashMap, HashSet},
    env,
    pin::Pin,
    sync::Arc,
    task::Poll,
    time::Duration,
};
use tokio::time::Timeout;

use crate::data::structs::{DmActivity, Error};

use lumi::serenity_prelude as serenity;

use std::ops::Deref;

use super::responses::{GuildCache, RegexData, ResponseCache, ResponseType};

macro_rules! id_wrapper {
    ($wrapper_name:ident, $inner_name:ident) => {
        #[derive(Clone, Copy, PartialEq, Debug)]
        pub struct $wrapper_name(pub $inner_name);

        impl Deref for $wrapper_name {
            type Target = $inner_name;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        /// Convert from i64
        impl From<i64> for $wrapper_name {
            fn from(item: i64) -> Self {
                $wrapper_name($inner_name::new(item as u64))
            }
        }
    };
}

id_wrapper!(UserIdWrapper, UserId);
id_wrapper!(ChannelIdWrapper, GenericChannelId);
id_wrapper!(MessageIdWrapper, MessageId);

/// Because don't we all hate the orphan rule.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MaybeMessageIdWrapper(pub Option<MessageIdWrapper>);

impl MaybeMessageIdWrapper {
    #[must_use]
    pub fn new(option: Option<MessageIdWrapper>) -> Self {
        MaybeMessageIdWrapper(option)
    }
}

impl From<Option<i64>> for MaybeMessageIdWrapper {
    fn from(option: Option<i64>) -> Self {
        MaybeMessageIdWrapper(option.map(MessageIdWrapper::from))
    }
}

impl Deref for MaybeMessageIdWrapper {
    type Target = Option<MessageIdWrapper>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub async fn init_data() -> Database {
    let database_url =
        env::var("DATABASE_URL").expect("No database url found in environment variables!");

    let database = PgPoolOptions::new()
        .connect(&database_url)
        .await
        .expect("Failed to connect to database!");

    database
        .execute("SET client_encoding TO 'UTF8'")
        .await
        .unwrap();

    sqlx::migrate!("../migrations")
        .run(&database)
        .await
        .expect("Unable to apply migrations!");

    let cache = mini_moka::sync::CacheBuilder::new(500)
        .time_to_idle(Duration::from_secs(3600))
        .build();

    Database {
        starboard: Mutex::new(
            StarboardHandler::new(&database)
                .await
                .expect("should be setup correctly."),
        ),
        db: database,
        users: cache,
        dm_activity: DashMap::new(),
        responses: ResponseCache::default(),
        channels: DashMap::new(),
        guilds: DashMap::new(),
    }
}

/// Custom type.
#[derive(Debug, Clone, sqlx::Type, PartialEq)]
#[sqlx(type_name = "emoteusagetype")]
pub enum EmoteUsageType {
    Message,
    Reaction,
}

pub struct Database {
    pub db: PgPool,
    users: mini_moka::sync::Cache<UserId, Arc<ApplicationUser>>,
    guilds: DashMap<serenity::GuildId, i32>,
    channels: DashMap<serenity::GenericChannelId, (i32, Option<i32>)>,
    pub starboard: Mutex<StarboardHandler>,
    // TODO: try and keep private and rewrite them when i eventually redo my users and starboard part.
    /// Runtime caches for dm activity.
    pub(crate) dm_activity: DashMap<UserId, DmActivity>,
    /// caches for regex autoresponse stuff.
    pub(crate) responses: ResponseCache,
}

#[bool_to_bitflags::bool_to_bitflags]
pub struct ApplicationUser {
    pub id: i32,
    is_banned: bool,
    is_admin: bool,
    pub allowed_admin_commands: Option<Vec<FixedString<u8>>>,
}

impl ApplicationUser {
    pub fn new(
        id: i32,
        is_banned: bool,
        is_admin: bool,
        allowed_admin_commands: Option<Vec<FixedString<u8>>>,
    ) -> Self {
        let mut app = Self {
            id,
            allowed_admin_commands,
            __generated_flags: ApplicationUserGeneratedFlags::empty(),
        };

        app.set_is_admin(is_admin);
        app.set_is_banned(is_banned);

        app
    }
}

/* pub struct Database {
    pub db: PgPool,
    banned_users: DashSet<UserId>,
    owner_overwrites: Checks,
    // TODO: return privacy
    pub starboard: Mutex<StarboardHandler>,

    /// Runtime caches for dm activity.
    pub(crate) dm_activity: DashMap<UserId, DmActivity>,

    /// caches for regex autoresponse stuff.
    pub(crate) responses: ResponseCache,
} */

#[derive(Debug)]
pub struct StarboardHandler {
    messages: Vec<StarboardMessage>,
    being_handled: HashSet<MessageId>,
    // message id is the appropriate in messages, the first userid is the author
    // the collection is the reaction users.
    pub reactions_cache: HashMap<MessageId, (UserId, Vec<UserId>)>,
    pub overrides: HashMap<GenericChannelId, u8>,
}

impl StarboardHandler {
    async fn new(db: &PgPool) -> Result<Self, Error> {
        let results = sqlx::query!("SELECT * FROM starboard_overrides")
            .fetch_all(db)
            .await?;

        let mut overrides = HashMap::with_capacity(results.len());
        for result in results {
            overrides.insert(
                GenericChannelId::new(result.channel_id as u64),
                result.star_count as u8,
            );
        }

        Ok(Self {
            overrides,
            messages: Vec::new(),
            being_handled: HashSet::new(),
            reactions_cache: HashMap::new(),
        })
    }
}
#[derive(Clone, Debug, Default)]
pub struct Checks {
    // Users under this will have access to all owner commands.
    pub owners_all: DashSet<UserId>,
    pub owners_single: DashMap<String, HashSet<UserId>>,
}

#[derive(Clone, Debug)]
pub struct StarboardMessage {
    pub id: i32,
    pub user_id: UserIdWrapper,
    pub username: String,
    pub avatar_url: Option<String>,
    pub content: String,
    pub channel_id: ChannelIdWrapper,
    pub message_id: MessageIdWrapper,
    pub attachment_urls: Vec<String>,
    pub star_count: i16,
    pub starboard_status: StarboardStatus,
    pub starboard_message_id: MessageIdWrapper,
    pub starboard_message_channel: ChannelIdWrapper,
    pub reply_message_id: MaybeMessageIdWrapper,
    pub reply_username: Option<String>,
    pub forwarded: bool,
}

#[derive(Debug, Clone, sqlx::Type, PartialEq)]
#[sqlx(type_name = "starboard_status")]
pub enum StarboardStatus {
    InReview,
    Accepted,
    Denied,
}

impl Database {
    pub async fn get_user(&self, user_id: serenity::UserId) -> Result<Arc<ApplicationUser>, Error> {
        if let Some(user) = self.users.get(&user_id) {
            return Ok(user);
        }

        self.insert_user_(user_id).await.map(Arc::new)
    }

    pub async fn insert_user_(&self, user_id: serenity::UserId) -> Result<ApplicationUser, Error> {
        let row = query!(
            r#"
            WITH input_rows(user_id) AS (
                VALUES ($1::bigint)
            ),
            ins AS (
                INSERT INTO users (user_id)
                SELECT user_id FROM input_rows
                ON CONFLICT (user_id) DO NOTHING
                RETURNING
                    id,
                    is_bot_banned,
                    is_bot_admin,
                    allowed_admin_commands
            )
            SELECT
                id AS "id!",
                is_bot_banned AS "is_bot_banned!",
                is_bot_admin AS "is_bot_admin!",
                allowed_admin_commands
            FROM ins
            UNION ALL
            SELECT
                u.id AS "id!",
                u.is_bot_banned AS "is_bot_banned!",
                u.is_bot_admin AS "is_bot_admin!",
                u.allowed_admin_commands
            FROM users u
            JOIN input_rows i USING (user_id);
            "#,
            user_id.get() as i64
        )
        .fetch_one(&self.db)
        .await?;

        let application_user = ApplicationUser::new(
            row.id,
            row.is_bot_banned,
            row.is_bot_admin,
            row.allowed_admin_commands.map(|i| {
                i.iter()
                    .map(|s| FixedString::from_str_trunc(s))
                    .collect::<Vec<_>>()
            }),
        );

        Ok(application_user)
    }

    /// Gets the channel from the database, or inserts it if it doesn't exist, returning the inner channel_id, and guild_id values.
    pub async fn get_channel(
        &self,
        channel_id: serenity::GenericChannelId,
        guild_id: Option<serenity::GuildId>,
    ) -> Result<(i32, Option<i32>), Error> {
        if let Some(id) = self.channels.get(&channel_id) {
            let value = id.value();
            return Ok((value.0, value.1));
        }

        self.insert_channel_(channel_id, guild_id).await
    }

    // inserts the channel and guild_id, returning both the inner ids if available.
    async fn insert_channel_(
        &self,
        channel_id: serenity::GenericChannelId,
        guild_id: Option<serenity::GuildId>,
    ) -> Result<(i32, Option<i32>), Error> {
        let inner_guild_id = if let Some(guild_id) = guild_id {
            Some(self.get_guild(guild_id).await?)
        } else {
            None
        };

        let row = query!(
            r#"
            WITH input_rows(channel_id, guild_id) AS (
                VALUES ($1::bigint, $2::bigint)
            ),
            ins AS (
                INSERT INTO channels (channel_id, guild_id)
                SELECT channel_id, guild_id FROM input_rows
                ON CONFLICT (channel_id) DO NOTHING
                RETURNING id
            )
            SELECT id AS "id!" FROM ins
            UNION ALL
            SELECT c.id AS "id!" FROM channels c
            JOIN input_rows i USING (channel_id);
            "#,
            channel_id.get() as i64,
            guild_id.map(|g| g.get() as i64),
        )
        .fetch_one(&self.db)
        .await?;

        Ok((row.id, inner_guild_id))
    }

    /// Gets the guild from the database, or inserts it if it doesn't exist, returning the inner id value.
    pub async fn get_guild(&self, guild_id: serenity::GuildId) -> Result<i32, Error> {
        if let Some(id) = self.guilds.get(&guild_id) {
            return Ok(*id.value());
        }

        self.insert_guild_(guild_id).await
    }

    async fn insert_guild_(&self, guild_id: serenity::GuildId) -> Result<i32, Error> {
        let row = query!(
            r#"
            WITH input_rows(guild_id) AS (
                VALUES ($1::bigint)
            ),
            ins AS (
                INSERT INTO guilds (guild_id)
                SELECT guild_id FROM input_rows
                ON CONFLICT (guild_id) DO NOTHING
                RETURNING id
            )
            SELECT id AS "id!" FROM ins
            UNION ALL
            SELECT g.id AS "id!" FROM guilds g
            JOIN input_rows i USING (guild_id);
        "#,
            guild_id.get() as i64
        )
        .fetch_one(&self.db)
        .await?;

        Ok(row.id)
    }

    /// Sets the user banned/unbanned from the bot, returning the old status.
    pub async fn set_banned(&self, user_id: UserId, set_banned: bool) -> Result<(), Error> {
        let is_cached = if let Some(cached_user) = self.users.get(&user_id) {
            if cached_user.is_banned() == set_banned {
                return Ok(());
            }

            true
        } else {
            false
        };

        if is_cached {
            query!(
                "UPDATE users SET is_bot_banned  = $1 WHERE user_id = $2",
                set_banned,
                user_id.get() as i64
            )
            .execute(&self.db)
            .await?;
        } else {
            let row = query!(
                r#"
                INSERT INTO users (user_id, is_bot_banned)
                VALUES ($1, $2)
                ON CONFLICT (user_id)
                DO UPDATE SET is_bot_banned = EXCLUDED.is_bot_banned
                RETURNING id, is_bot_admin, allowed_admin_commands
                "#,
                user_id.get() as i64,
                set_banned
            )
            .fetch_one(&self.db)
            .await?;

            let application_user = ApplicationUser::new(
                row.id,
                set_banned,
                row.is_bot_admin,
                row.allowed_admin_commands.map(|i| {
                    i.iter()
                        .map(|s| FixedString::from_str_trunc(s))
                        .collect::<Vec<_>>()
                }),
            );

            self.users.insert(user_id, Arc::new(application_user));
        }

        Ok(())
    }

    /// To be called in a function that uses the owner check.
    #[must_use]
    pub async fn check_owner(&self, user_id: UserId, command: &str) -> Result<bool, Error> {
        if let Some(cached) = self.users.get(&user_id) {
            if cached.is_admin() {
                return Ok(true);
            }

            if let Some(commands) = &cached.allowed_admin_commands {
                return Ok(commands.iter().any(|c| c.as_str() == command));
            }
        };

        // TODO: query database

        Ok(false)
    }

    /// Sets or unsets a user's admin access to this bot.
    pub async fn set_admin(
        &self,
        user_id: UserId,
        command: Option<&str>,
        enable: bool,
    ) -> Result<bool, Error> {
        let Some(command) = command else {
            if let Some(cached) = self.users.get(&user_id) {
                if cached.is_admin() == enable {
                    return Ok(true);
                }
            }

            sqlx::query!(
                r#"
            INSERT INTO users (user_id, is_bot_admin)
            VALUES ($1, $2)
            ON CONFLICT (user_id)
            DO UPDATE SET is_bot_admin = EXCLUDED.is_bot_admin
            "#,
                user_id.get() as i64,
                enable
            )
            .execute(&self.db)
            .await?;

            return Ok(true);
        };

        // If command is present, handle add/remove
        if let Some(cached) = self.users.get(&user_id) {
            let already_has = cached
                .allowed_admin_commands
                .as_ref()
                .map(|array| array.iter().any(|c| c == command))
                .unwrap_or(false);

            if already_has == enable {
                return Ok(true);
            }
        }

        if enable {
            // Add command if not present
            sqlx::query!(
                r#"
            INSERT INTO users (user_id, allowed_admin_commands)
            VALUES ($1, ARRAY[$2])
            ON CONFLICT (user_id)
            DO UPDATE SET allowed_admin_commands =
                CASE
                    WHEN NOT $2 = ANY(users.allowed_admin_commands) THEN
                        array_append(users.allowed_admin_commands, $2)
                    ELSE users.allowed_admin_commands
                END
            "#,
                user_id.get() as i64,
                command
            )
            .execute(&self.db)
            .await?;
        } else {
            // Remove command if present
            sqlx::query!(
                r#"
            INSERT INTO users (user_id, allowed_admin_commands)
            VALUES ($1, ARRAY[]::TEXT[])
            ON CONFLICT (user_id)
            DO UPDATE SET allowed_admin_commands = array_remove(users.allowed_admin_commands, $2)
            "#,
                user_id.get() as i64,
                command
            )
            .execute(&self.db)
            .await?;
        }

        Ok(false)
    }

    pub async fn get_starboard_msg(&self, msg_id: MessageId) -> Result<StarboardMessage, Error> {
        if let Some(starboard) = self
            .starboard
            .lock()
            .messages
            .iter()
            .find(|s| *s.message_id == msg_id)
            .cloned()
        {
            return Ok(starboard);
        }

        let starboard = self.get_starboard_msg_(msg_id).await?;

        self.starboard.lock().messages.push(starboard.clone());

        Ok(starboard)
    }

    async fn get_starboard_msg_(&self, msg_id: MessageId) -> Result<StarboardMessage, sqlx::Error> {
        sqlx::query_as!(
            StarboardMessage,
            r#"
            SELECT
                s.id,
                u.user_id,
                s.username,
                s.avatar_url,
                s.content,
                c.channel_id,
                m.message_id,
                s.attachment_urls,
                s.star_count,
                sm.message_id AS starboard_message_id,
                sc.channel_id AS starboard_message_channel,
                s.starboard_status as "starboard_status: StarboardStatus",
                rm.message_id AS "reply_message_id?",
                s.forwarded,
                s.reply_username
            FROM starboard s
            JOIN users u ON s.user_id = u.id
            JOIN messages m ON s.message_id = m.message_id
            JOIN channels c ON m.channel_id = c.id

            LEFT JOIN messages rm ON s.reply_message_id = rm.message_id
            LEFT JOIN messages sm ON s.starboard_message_id = sm.message_id
            LEFT JOIN channels sc ON s.starboard_message_channel = sc.id

            WHERE s.message_id = $1
            "#,
            msg_id.get() as i64
        )
        .fetch_one(&self.db)
        .await
    }

    pub async fn update_star_count(&self, id: i32, count: i16) -> Result<(), sqlx::Error> {
        {
            let mut starboard = self.starboard.lock();
            let entry = starboard.messages.iter_mut().find(|s| s.id == id);
            if let Some(entry) = entry {
                entry.star_count = count;
            }
        };

        query!(
            "UPDATE starboard SET star_count = $1 WHERE id = $2",
            count,
            id,
        )
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn get_responses_regexes(
        &self,
        guild_id: serenity::GuildId,
    ) -> Result<Option<GuildCache>, Error> {
        if let Some(cache) = self.responses.guild.get(&guild_id) {
            return Ok(Some(cache.clone()));
        }

        let all_data = sqlx::query!(
            "SELECT
                r.id AS regex_id, r.channel_id, r.pattern,
                r.recurse_channels, r.recurse_threads, r.detection_type,
                resp.message, resp.emote_id
            FROM regexes r
            LEFT JOIN responses resp ON r.id = resp.regex_id
            WHERE r.guild_id = $1",
            guild_id.get() as i64
        )
        .fetch_all(&self.db)
        .await?;

        let mut guild_cache = GuildCache::default();
        let mut emoji_cache: HashMap<i32, ReactionType> = HashMap::new();

        for record in &all_data {
            let exceptions = sqlx::query!(
                "SELECT channel_id FROM regex_exceptions WHERE regex_id = $1",
                record.regex_id
            )
            .fetch_all(&self.db)
            .await
            .unwrap();

            let exception_channels = exceptions
                .into_iter()
                .map(|row| GenericChannelId::new(row.channel_id as u64))
                .collect::<HashSet<_>>();

            let response = if let Some(message) = &record.message {
                ResponseType::Message(message.clone())
            } else if let Some(emote_id) = record.emote_id {
                let reaction = if let Some(cached_reaction) = emoji_cache.get(&emote_id) {
                    cached_reaction.clone()
                } else {
                    let emote = sqlx::query!(
                        "SELECT emote_name, discord_id FROM emotes WHERE id = $1",
                        emote_id
                    )
                    .fetch_one(&self.db)
                    .await
                    .unwrap();

                    let reaction = if let Some(discord_id) = emote.discord_id {
                        ReactionType::Custom {
                            animated: false,
                            id: serenity::EmojiId::new(discord_id as u64),
                            name: Some(small_fixed_array::FixedString::from_static_trunc("_")),
                        }
                    } else {
                        ReactionType::Unicode(
                            serenity::small_fixed_array::FixedString::from_string_trunc(
                                emote.emote_name,
                            ),
                        )
                    };

                    emoji_cache.insert(emote_id, reaction.clone());

                    reaction
                };

                ResponseType::Emoji(reaction)
            } else {
                ResponseType::Message("Default response".to_string())
            };

            let regex_data = RegexData {
                id: record.regex_id,
                pattern: Regex::new(&record.pattern).unwrap(),
                recurse_channels: record.recurse_channels,
                recurse_threads: record.recurse_threads,
                response,
                exceptions: exception_channels,
                detection_type: (record.detection_type as u8).into(),
            };

            if let Some(channel_id) = record.channel_id {
                guild_cache
                    .channel
                    .entry(GenericChannelId::new(channel_id as u64))
                    .or_insert_with(Vec::new)
                    .push(regex_data);
            } else {
                guild_cache.global.push(regex_data);
            }
        }

        self.responses.guild.insert(guild_id, guild_cache);

        // TODO: figure out an ergonomic way that i could run OCR without deadlocking if i don't clone.
        Ok(self.responses.guild.get(&guild_id).map(|g| g.clone()))
    }

    /// Check if a starboard is being handled, and if its not, handle it.
    ///
    /// returns if its already being handled.
    pub fn handle_starboard(&self, message_id: MessageId) -> bool {
        !self.starboard.lock().being_handled.insert(message_id)
    }

    /// Remove the safety check for a starboard being handled.
    pub fn stop_handle_starboard(&self, message_id: &MessageId) {
        self.starboard.lock().being_handled.remove(message_id);
    }

    pub async fn update_starboard_fields(&self, m: &StarboardMessage) -> Result<(), Error> {
        sqlx::query!(
            r#"
        UPDATE starboard
        SET
            content = $1,
            attachment_urls = $2,
            starboard_status = $3
        WHERE message_id = $4
        "#,
            m.content,
            &m.attachment_urls,
            m.starboard_status as _,
            m.message_id.get() as i64,
        )
        .execute(&self.db)
        .await?;

        let mut lock = self.starboard.lock();
        let index = lock
            .messages
            .iter()
            .position(|cache| cache.message_id.0 == m.message_id.0);

        if let Some(index) = index {
            lock.messages.remove(index);
        }

        Ok(())
    }

    pub async fn insert_starboard_msg(
        &self,
        m: StarboardMessage,
        guild_id: Option<serenity::GuildId>,
    ) -> Result<(), sqlx::Error> {
        let m_id = *m.message_id;
        let _ = self.insert_starboard_msg_(m, guild_id).await;
        self.stop_handle_starboard(&m_id);

        Ok(())
    }

    async fn insert_starboard_msg_(
        &self,
        mut m: StarboardMessage,
        guild_id: Option<serenity::GuildId>,
    ) -> Result<(), Error> {
        if let Some(g_id) = guild_id {
            self.get_guild(g_id).await?;
        }

        let origin_channel = self.get_channel(*m.channel_id, guild_id).await?.0;
        let message_channel = self
            .get_channel(*m.starboard_message_channel, guild_id)
            .await?
            .0;

        let user = self.get_user(*m.user_id).await?.id;

        let transaction = self.db.begin().await?;

        let val = match sqlx::query!(
            r#"
                INSERT INTO starboard (
                    user_id, username, avatar_url, content, channel_id, message_id,
                    attachment_urls, star_count, starboard_status,
                    starboard_message_id, starboard_message_channel, forwarded, reply_message_id, reply_username
                )
                VALUES (
                    $1, $2, $3, $4, $5, $6,
                    $7, $8, $9, $10, $11,
                    $12, $13, $14
                ) RETURNING id
                "#,
            m.user_id.get() as i64,
            m.username,
            m.avatar_url,
            m.content,
            m.channel_id.get() as i64,
            m.message_id.get() as i64,
            &m.attachment_urls,
            m.star_count,
            m.starboard_status as _,
            m.starboard_message_id.get() as i64,
            m.starboard_message_channel.get() as i64,
            m.forwarded,
            m.reply_message_id.map(|m| m.get() as i64),
            m.reply_username
        )
        .fetch_one(&self.db)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                println!("SQL query failed: {e:?}");
                return Err(e.into());
            }
        };

        m.id = val.id;

        let mut lock = self.starboard.lock();
        let m_id = *m.message_id;

        lock.messages.push(m);
        lock.being_handled.remove(&m_id);

        Ok(())
    }

    pub async fn get_starboard_msg_by_starboard_id(
        &self,
        starboard_msg_id: MessageId,
    ) -> Result<StarboardMessage, Error> {
        if let Some(starboard) = self
            .starboard
            .lock()
            .messages
            .iter()
            .find(|s| *s.starboard_message_id == starboard_msg_id)
            .cloned()
        {
            return Ok(starboard);
        }

        let starboard = self
            .get_starboard_msg_by_starboard_id_(starboard_msg_id)
            .await?;

        self.starboard.lock().messages.push(starboard.clone());

        Ok(starboard)
    }

    // TODO: test
    async fn get_starboard_msg_by_starboard_id_(
        &self,
        starboard_msg_id: MessageId,
    ) -> Result<StarboardMessage, sqlx::Error> {
        sqlx::query_as!(
            StarboardMessage,
            r#"
            SELECT
                s.id,
                u.user_id,
                s.username,
                s.avatar_url,
                s.content,
                c.channel_id,
                m.message_id,
                s.attachment_urls,
                s.star_count,
                sm.message_id AS starboard_message_id,
                sc.channel_id AS starboard_message_channel,
                s.starboard_status as "starboard_status: StarboardStatus",
                rm.message_id AS "reply_message_id?",
                s.forwarded,
                s.reply_username
            FROM starboard s
            JOIN users u ON s.user_id = u.id
            JOIN messages m ON s.message_id = m.message_id
            JOIN channels c ON m.channel_id = c.id
            LEFT JOIN messages rm ON s.reply_message_id = rm.message_id
            LEFT JOIN messages sm ON s.starboard_message_id = sm.message_id
            LEFT JOIN channels sc ON s.starboard_message_channel = sc.id
            WHERE s.starboard_message_id = $1
            "#,
            starboard_msg_id.get() as i64
        )
        .fetch_one(&self.db)
        .await
    }

    pub async fn approve_starboard(
        &self,
        starboard_message_id: MessageId,
        new_message_id: MessageId,
        new_channel_id: GenericChannelId,
    ) -> Result<(), Error> {
        let status = StarboardStatus::Accepted;

        query!(
            "UPDATE starboard SET starboard_status = $1, starboard_message_id = $2, \
             starboard_message_channel = $3 WHERE starboard_message_id = $4",
            status as _,
            new_message_id.get() as i64,
            new_channel_id.get() as i64,
            starboard_message_id.get() as i64,
        )
        .execute(&self.db)
        .await?;

        let mut lock = self.starboard.lock();
        let m = lock
            .messages
            .iter_mut()
            .find(|m| *m.starboard_message_id == starboard_message_id);

        if let Some(m) = m {
            m.starboard_message_channel = ChannelIdWrapper(new_channel_id);
            m.starboard_message_id = MessageIdWrapper(new_message_id);
            m.starboard_status = StarboardStatus::Accepted;
        }

        Ok(())
    }

    pub async fn deny_starboard(&self, starboard_message_id: MessageId) -> Result<(), Error> {
        let status = StarboardStatus::Denied;

        query!(
            "UPDATE starboard SET starboard_status = $1 WHERE starboard_message_id = $2",
            status as _,
            starboard_message_id.get() as i64,
        )
        .execute(&self.db)
        .await?;

        let mut lock = self.starboard.lock();
        if let Some(index) = lock
            .messages
            .iter()
            .position(|m| *m.starboard_message_id == starboard_message_id)
        {
            lock.messages.remove(index);
        }

        Ok(())
    }

    pub async fn get_all_starboard(&self) -> Result<Vec<StarboardMessage>, Error> {
        let messages = sqlx::query_as!(
            StarboardMessage,
            r#"
            SELECT
                s.id,
                u.user_id,
                s.username,
                s.avatar_url,
                s.content,
                c.channel_id,
                m.message_id,
                s.attachment_urls,
                s.star_count,
                sm.message_id AS starboard_message_id,
                sc.channel_id AS starboard_message_channel,
                s.starboard_status as "starboard_status: StarboardStatus",
                rm.message_id AS "reply_message_id?",
                s.forwarded,
                s.reply_username
            FROM starboard s
            JOIN users u ON s.user_id = u.id
            JOIN messages m ON s.message_id = m.message_id
            JOIN channels c ON m.channel_id = c.id

            JOIN messages rm ON s.reply_message_id = rm.message_id
            JOIN messages sm ON s.starboard_message_id = sm.message_id
            JOIN channels sc ON s.starboard_message_channel = sc.id

            "#,
        )
        .fetch_all(&self.db)
        .await?;

        let mut guard = self.starboard.lock();

        for message in messages {
            if !guard.messages.iter().any(|m| m.id == message.id) {
                guard.messages.push(message);
            }
        }

        Ok(guard.messages.clone())
    }

    pub async fn add_starboard_override(
        &self,
        starboard_handler: &Mutex<StarboardHandler>,
        channel_id: GenericChannelId,
        starcount: u8,
    ) -> Result<(), Error> {
        // TODO: use starboard config directly to avoid panic
        self.get_channel(
            channel_id,
            std::env::var("STARBOARD_GUILD")
                .unwrap()
                .parse::<u64>()
                .map(std::convert::Into::into)
                .ok(),
        )
        .await?;

        sqlx::query!(
            r#"
            INSERT INTO starboard_overrides (channel_id, star_count)
            VALUES ($1, $2)
            ON CONFLICT (channel_id) DO UPDATE
            SET star_count = EXCLUDED.star_count
            "#,
            channel_id.get() as i64,
            i16::from(starcount)
        )
        .execute(&self.db)
        .await?;

        starboard_handler
            .lock()
            .overrides
            .insert(channel_id, starcount);

        Ok(())
    }

    pub async fn remove_starboard_override(
        &self,
        starboard_handler: &Mutex<StarboardHandler>,
        channel_id: GenericChannelId,
    ) -> Result<bool, Error> {
        let result = sqlx::query!(
            "DELETE FROM starboard_overrides WHERE channel_id = $1",
            channel_id.get() as i64
        )
        .execute(&self.db)
        .await?;

        if result.rows_affected() == 0 {
            return Ok(false);
        }

        starboard_handler.lock().overrides.remove(&channel_id);

        Ok(true)
    }

    pub async fn get_gamemode(&self, user_id: UserId, osu_id: u32) -> Result<GameMode, Error> {
        let res = query!(
            "SELECT gamemode FROM verified_users WHERE user_id = $1 AND osu_id = $2",
            user_id.get() as i64,
            osu_id as i32
        )
        .fetch_one(&self.db)
        .await?;

        Ok((res.gamemode as u8).into())
    }

    pub async fn inactive_user(&self, user_id: UserId) -> Result<(), Error> {
        Ok(query!(
            "UPDATE verified_users SET is_active = FALSE WHERE user_id = $1",
            user_id.get() as i64,
        )
        .execute(&self.db)
        .await
        .map(|_| ())?)
    }

    pub async fn update_last_updated(
        &self,
        user_id: UserId,
        time: i64,
        rank: Option<Option<u32>>,
        map_status: u8,
        roles: &[RoleId],
    ) -> Result<(), Error> {
        if let Some(rank) = rank {
            query!(
                "UPDATE verified_users SET last_updated = $2, rank = $3, map_status = $4, \
                 verified_roles = $5 WHERE user_id = $1",
                user_id.get() as i64,
                time,
                rank.map(|r| r as i32),
                i16::from(map_status),
                &roles.iter().map(|r| r.get() as i64).collect::<Vec<_>>(),
            )
            .execute(&self.db)
            .await?;
        } else {
            query!(
                "UPDATE verified_users SET last_updated = $2, map_status = $3, verified_roles = \
                 $4 WHERE user_id = $1",
                user_id.get() as i64,
                time,
                i16::from(map_status),
                &roles.iter().map(|r| r.get() as i64).collect::<Vec<_>>(),
            )
            .execute(&self.db)
            .await?;
        }

        Ok(())
    }

    pub async fn verify_user(&self, user_id: UserId, osu_id: u32) -> Result<(), Error> {
        let now = Utc::now().timestamp();

        self.insert_user(user_id).await?;

        query!(
            r#"
            INSERT INTO verified_users (user_id, osu_id, last_updated, is_active, gamemode)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (user_id)
            DO UPDATE SET
                last_updated = EXCLUDED.last_updated,
                is_active = EXCLUDED.is_active,
                gamemode = 0
            "#,
            user_id.get() as i64,
            osu_id as i32,
            now,
            true,
            0
        )
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn unlink_user(&self, user_id: UserId) -> Result<u32, Error> {
        let record = query!(
            "DELETE FROM verified_users WHERE user_id = $1 RETURNING osu_id",
            user_id.get() as i64
        )
        .fetch_optional(&self.db)
        .await?;

        Ok(record.unwrap().osu_id as u32)
    }

    pub async fn get_osu_user_id(&self, user_id: UserId) -> Option<(u32, GameMode)> {
        let query = query!(
            "SELECT osu_id, gamemode FROM verified_users WHERE user_id = $1",
            user_id.get() as i64
        )
        .fetch_one(&self.db)
        .await
        .ok()?;

        Some((query.osu_id as u32, (query.gamemode as u8).into()))
    }

    pub async fn change_mode(&self, user_id: UserId, gamemode: GameMode) -> Result<(), Error> {
        query!(
            "UPDATE verified_users SET gamemode = $1 WHERE user_id = $2",
            gamemode as i16,
            user_id.get() as i64
        )
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn get_existing_links(&self, osu_id: u32) -> Result<Vec<UserId>, sqlx::Error> {
        sqlx::query_scalar!(
            "SELECT user_id FROM verified_users WHERE osu_id = $1",
            osu_id as i32
        )
        .fetch_all(&self.db)
        .await
        .map(|u| {
            u.into_iter()
                .map(|u| UserId::new(u as u64))
                .collect::<Vec<UserId>>()
        })
    }

    // temporary function to give access to the inner command overwrites while i figure something out.
    // TODO: stop this lamo
    #[must_use]
    pub fn inner_overwrites(&self) -> Vec<Arc<ApplicationUser>> {
        self.users
            .iter()
            .filter(|u| u.is_admin() | u.allowed_admin_commands.is_some())
            .map(|c| c.clone())
            .collect()
    }
}

pub struct WaitForOsuAuth {
    pub state: u8,
    fut: Pin<Box<Timeout<tokio::sync::oneshot::Receiver<UserExtended>>>>,
}
pub enum AuthenticationStandbyError {
    Canceled,
    Timeout,
}

impl Future for WaitForOsuAuth {
    type Output = Result<UserExtended, AuthenticationStandbyError>;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match self.fut.poll_unpin(cx) {
            Poll::Ready(Ok(Ok(user))) => Poll::Ready(Ok(user)),
            Poll::Ready(Ok(Err(_))) => Poll::Ready(Err(AuthenticationStandbyError::Canceled)),
            Poll::Ready(Err(_)) => Poll::Ready(Err(AuthenticationStandbyError::Timeout)),
            Poll::Pending => Poll::Pending,
        }
    }
}
