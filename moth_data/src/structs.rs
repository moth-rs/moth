use dashmap::DashMap;
use parking_lot::RwLock;
use std::{collections::HashMap, time::Instant};

use poise::serenity_prelude::{ChannelId, GuildId, MessageId, UserId};

use std::sync::atomic::AtomicBool;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;
pub type PrefixContext<'a> = poise::PrefixContext<'a, Data, Error>;
pub type Command = poise::Command<Data, Error>;

pub struct Data {
    /// If the bots startup has been handled in the `on_ready` event.
    pub has_started: AtomicBool,
    /// Time the bot started.
    pub time_started: std::time::Instant,
    /// Wrapper for the bots database with helper functions.
    pub database: crate::database::Database,
    /// Http client.
    pub reqwest: reqwest::Client,
    /// Bot/Server Configuration
    pub config: RwLock<moth_config::MothConfig>,
    /// Experimental anti mass message deletion tracking.
    pub anti_delete_cache: AntiDeleteCache,
    pub starboard_config: StarboardConfig,
}

/// A struct only used to track if an error comes from a cooldown.
pub struct InvocationData {
    pub cooldown_remaining: Option<std::time::Duration>,
}

pub struct StarboardConfig {
    pub active: bool,
    /// The review queue channel.
    pub queue_channel: ChannelId,
    /// The channel to post the starboard in once reviewed.
    pub post_channel: ChannelId,
    /// The star emoji to look for.
    pub star_emoji: String,
    /// The single guild the starboard is configured for.
    pub guild_id: GuildId,
    pub threshold: u8,
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
            i64::from(user_id)
        )
        .fetch_one(&self.database.db)
        .await;

        match result {
            Ok(record) => Some(DmActivity::new(
                record.last_announced.unwrap(),
                record.until,
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
            until,
            count,
            i64::from(user_id)
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
            i64::from(user_id),
            announced,
            until,
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
            i64::from(user_id)
        )
        .execute(&self.database.db)
        .await;
    }
}
