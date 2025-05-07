use dashmap::DashMap;
use moth_core::data::structs::{Data, StarboardConfig, WebServer};
use serenity::all::{GenericChannelId, GuildId, RoleId};
use std::sync::{atomic::AtomicBool, Arc};

pub async fn setup() -> Arc<Data> {
    let handler = moth_core::data::database::init_data().await;

    let config = moth_core::config::MothConfig::load_config();
    let starboard_config = starboard_config();

    Arc::new(Data {
        has_started: AtomicBool::new(false),
        database: handler,
        time_started: std::time::Instant::now(),
        reqwest: reqwest::Client::new(),
        config: parking_lot::RwLock::new(config),
        anti_delete_cache: moth_core::data::structs::AntiDeleteCache::default(),
        starboard_config,
        ocr_engine: moth_core::ocr::OcrEngine::new(),
        new_join_vc: DashMap::default(),
        web: WebServer::new().await,
    })
}

macro_rules! get_env_or_default {
    ($var_name:expr, $kind:ty, $default:expr) => {
        std::env::var($var_name)
            .ok()
            .and_then(|val| val.parse::<$kind>().ok())
            .unwrap_or_else(|| <$kind>::new($default))
    };
}

fn starboard_config() -> StarboardConfig {
    StarboardConfig {
        active: std::env::var("STARBOARD_ACTIVE")
            .map(|e| e.parse::<bool>().unwrap())
            .unwrap_or(true),

        queue_channel: get_env_or_default!(
            "STARBOARD_QUEUE",
            GenericChannelId,
            1324543000600383549
        ),
        post_channel: get_env_or_default!(
            "STARBOARD_CHANNEL",
            GenericChannelId,
            1324437745854316564
        ),
        guild_id: get_env_or_default!("STARBOARD_GUILD", GuildId, 98226572468690944),
        allowed_role: get_env_or_default!("STARBOARD_ROLE", RoleId, 98459030455853056),
        star_emoji: std::env::var("STARBOARD_EMOJI").unwrap_or("⭐".to_owned()),
        threshold: std::env::var("STARBOARD_THRESHOLD")
            .ok()
            .and_then(|val| val.parse::<u8>().ok())
            .unwrap_or(5),
    }
}
