use chrono::TimeZone;
use dashmap::DashMap;
use parking_lot::{Mutex, RwLock};
use rosu_v2::Osu;
use serenity::all::{ChannelId, Member, RoleId, SecretString};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::Instant,
};

use lumi::serenity_prelude::{GenericChannelId, GuildId, MessageId, UserId};

use std::sync::atomic::AtomicBool;

use crate::{
    standby::AuthenticationStandby,
    verification::{sender::VerificationSender, task},
};

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = lumi::Context<'a, Data, Error>;
pub type PrefixContext<'a> = lumi::PrefixContext<'a, Data, Error>;
pub type ApplicationContext<'a> = lumi::ApplicationContext<'a, Data, Error>;
pub type FrameworkContext<'a> = lumi::FrameworkContext<'a, Data, Error>;
pub type Command = lumi::Command<Data, Error>;

pub struct Data {
    /// If the bots startup has been handled in the `on_ready` event.
    pub has_started: AtomicBool,
    /// Time the bot started.
    pub time_started: std::time::Instant,
    /// Wrapper for the bots database with helper functions.
    pub database: crate::data::database::Database,
    /// Http client.
    pub reqwest: reqwest::Client,
    /// Bot/Server Configuration
    pub config: RwLock<crate::config::MothConfig>,
    /// Experimental anti mass message deletion tracking.
    pub anti_delete_cache: AntiDeleteCache,
    pub starboard_config: StarboardConfig,
    pub ocr_engine: crate::ocr::OcrEngine,
    /// ugh
    pub new_join_vc: DashMap<UserId, Fuck>,
    /// ugh 2
    pub osu_game_joins: Mutex<VecDeque<UserId>>,
    pub web: WebServer,
    pub auto_pooped: dashmap::DashSet<UserId>,
}

pub struct WebServer {
    pub osu: Osu,
    pub handlebars: handlebars::Handlebars<'static>,
    pub osu_client_id: u64,
    pub osu_client_secret: SecretString,
    pub task_sender: VerificationSender,
    pub auth_standby: AuthenticationStandby,
}

impl Drop for WebServer {
    fn drop(&mut self) {
        let sender = self.task_sender.clone();
        tokio::spawn(async move {
            sender.shutdown().await;
        });
    }
}

impl WebServer {
    pub async fn new() -> Self {
        let client_id = std::env::var("CLIENT_ID").unwrap().parse::<u64>().unwrap();
        let client_secret = std::env::var("CLIENT_SECRET").unwrap();

        let mut handlebars = handlebars::Handlebars::new();
        handlebars
            .register_template_file("index", "./web/auth/index.hbs")
            .expect("Failed to register template");

        Self {
            osu: Osu::new(client_id, client_secret.clone()).await.unwrap(),
            osu_client_id: client_id,
            osu_client_secret: SecretString::new(client_secret.into()),
            handlebars,
            task_sender: VerificationSender::new(None),
            auth_standby: AuthenticationStandby::new(),
        }
    }

    /// Starts the background task, will run regardless of if an existing task is running (will not be dropped)
    ///
    /// Populates the task sender.
    pub async fn start_background_task(&self, ctx: serenity::all::Context) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.task_sender.set(tx).await;

        tokio::spawn(async move {
            task(ctx, rx).await;
        });
    }
}

#[derive(Clone, Debug)]
pub struct Fuck {
    pub member: Member,
    pub channels: HashSet<ChannelId>,
    pub cleared: bool,
    pub announce_msg: Option<MessageId>,
}

/// A struct only used to track if an error comes from a cooldown.
pub struct InvocationData {
    pub cooldown_remaining: Option<std::time::Duration>,
}

pub struct StarboardConfig {
    pub active: bool,
    /// The review queue channel.
    pub queue_channel: GenericChannelId,
    /// The channel to post the starboard in once reviewed.
    pub post_channel: GenericChannelId,
    /// The star emoji to look for.
    pub star_emoji: String,
    /// The single guild the starboard is configured for.
    pub guild_id: GuildId,
    pub allowed_role: RoleId,
    pub threshold: u8,
}

impl StarboardConfig {
    #[must_use]
    pub fn emoji_fixed_string(&self) -> serenity::small_fixed_array::FixedString {
        serenity::small_fixed_array::FixedString::from_str_trunc(&self.star_emoji)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DmActivity {
    pub last_announced: i64,
    pub until: Option<i64>,
    pub count: i16,
}

impl DmActivity {
    #[must_use]
    pub fn new(last_announced: i64, until: Option<i64>, count: i16) -> Self {
        DmActivity {
            last_announced,
            until,
            count,
        }
    }
}

#[derive(Default)]
pub struct AntiDeleteCache {
    pub val: DashMap<GuildId, Decay>,
    // Dashmap using guild key, containing the last deleted msg and a hashmap of stored message ids.
    pub map: DashMap<GuildId, InnerCache>,
}
pub struct InnerCache {
    pub last_deleted_msg: MessageId,
    pub msg_user_cache: HashMap<MessageId, UserId>,
}
pub struct Decay {
    pub val: u16,
    pub last_update: Instant,
}

impl AntiDeleteCache {
    /// Check if all values should be decayed and if so, decay them.
    pub fn decay_proc(&self) {
        let now = Instant::now();
        let mut to_remove = vec![];
        for mut entry in self.val.iter_mut() {
            let guild = entry.value_mut();
            let elapsed = now.duration_since(guild.last_update).as_secs();
            // time without messages deleted to decay, hardcoded currently.
            if elapsed > 5 {
                guild.val -= 1;
            }
            if guild.val == 0 {
                to_remove.push(*entry.key());
            }
        }
        for entry in to_remove {
            self.val.remove(&entry);
        }
    }
}

#[allow(clippy::missing_panics_doc)]
impl Data {
    pub async fn get_activity_check(&self, user_id: UserId) -> Option<DmActivity> {
        let cached = self.database.dm_activity.get(&user_id);

        if let Some(cached) = cached {
            Some(*cached)
        } else {
            self.get_activity_check_psql(user_id).await
        }
    }

    async fn get_activity_check_psql(&self, user_id: UserId) -> Option<DmActivity> {
        let result = sqlx::query!(
            "SELECT last_announced, until, count FROM dm_activity WHERE user_id = $1",
            // TODO: check if i should be joining here or if pulling from cache is actually faster.
            match self.database.get_user(user_id).await {
                Ok(user) => user.id,
                Err(_) => return None,
            }
        )
        .fetch_one(&self.database.db)
        .await;

        match result {
            Ok(record) => Some(DmActivity::new(
                record.last_announced.unwrap().timestamp(),
                record.until.map(|t| t.timestamp()),
                record.count.unwrap(),
            )),
            Err(err) => {
                if let sqlx::Error::RowNotFound = err {
                    None
                } else {
                    tracing::warn!("Error when attempting to find row: {err}");
                    None
                }
            }
        }
    }

    pub async fn updated_no_announce(
        &self,
        user_id: UserId,
        announced: i64,
        until: i64,
        count: i16,
    ) {
        // count will have already been incremented.
        let _ = sqlx::query!(
            "UPDATE dm_activity SET until = $1, count = $2 WHERE user_id = $3",
            chrono::Utc.timestamp_opt(until, 0).unwrap(),
            count,
            match self.database.get_user(user_id).await {
                Ok(user) => user.id,
                Err(_) => return,
            }
        )
        .execute(&self.database.db)
        .await;

        self.update_user_cache(user_id, announced, until, count);
    }

    pub async fn new_or_announced(
        &self,
        user_id: UserId,
        announced: i64,
        until: i64,
        count: Option<i16>,
    ) {
        // If this is an update, count will have already been supplied and incremented.
        let _ = sqlx::query!(
            "INSERT INTO dm_activity (user_id, last_announced, until, count)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (user_id) DO UPDATE
            SET last_announced = $2, until = $3, count = $4",
            match self.database.get_user(user_id).await {
                Ok(user) => user.id,
                Err(_) => return,
            },
            chrono::Utc.timestamp_opt(announced, 0).unwrap(),
            chrono::Utc.timestamp_opt(until, 0).unwrap(),
            count.unwrap_or(0)
        )
        .execute(&self.database.db)
        .await;

        self.update_user_cache(user_id, announced, until, count.unwrap_or(0));
    }

    pub fn remove_dm_activity_cache(&self, user_id: UserId) {
        self.database.dm_activity.remove(&user_id);
    }

    fn update_user_cache(&self, user_id: UserId, announced: i64, until: i64, count: i16) {
        self.database
            .dm_activity
            .insert(user_id, DmActivity::new(announced, Some(until), count));
    }

    pub async fn remove_until(&self, user_id: UserId) {
        self.remove_dm_activity_cache(user_id);
        let _ = sqlx::query!(
            "UPDATE dm_activity SET until = NULL WHERE user_id = $1",
            match self.database.get_user(user_id).await {
                Ok(user) => user.id,
                Err(_) => return,
            }
        )
        .execute(&self.database.db)
        .await;
    }
}
