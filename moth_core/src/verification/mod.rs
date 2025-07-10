use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::data::structs::Data;
use axum::{
    Router,
    extract::{Query, State, rejection::QueryRejection},
    http::StatusCode,
    response::Html,
    routing::get,
};
use chrono::Utc;
use roles::{LOG_CHANNEL, MetadataType, UserMapHolder, maybe_update};
use rosu_v2::{Osu, prelude::GameMode};
use sender::VerificationCommand;
use serenity::{
    all::{CreateMessage, RoleId, UserId},
    futures::StreamExt,
};

use tokio_util::time::{
    DelayQueue,
    delay_queue::{Expired, Key},
};
use tower_http::cors::CorsLayer;

pub mod roles;
pub mod sender;

#[derive(serde::Deserialize)]
struct Params {
    state: u8,
    code: String,
}

pub async fn run(data: Arc<Data>) {
    let cors = CorsLayer::permissive();

    let app = Router::new()
        .route("/", get(auth_osu))
        .with_state(data)
        .layer(cors);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    axum::serve(listener, app).await.unwrap();
}

#[derive(serde::Serialize)]
struct IndexContext<'a> {
    success: bool,
    user: Option<&'a str>,
}

async fn auth_osu(
    query: Result<Query<Params>, QueryRejection>,
    State(state): State<Arc<Data>>,
) -> Result<(StatusCode, Html<String>), StatusCode> {
    if let Ok(page) = auth(query, &state).await {
        return Ok((StatusCode::OK, Html(page)));
    }

    let context = IndexContext {
        success: false,
        user: None,
    };

    let page = state
        .web
        .handlebars
        .render("index", &context)
        .expect("Failed to render template");

    Ok((StatusCode::OK, Html(page)))
}

async fn auth(query: Result<Query<Params>, QueryRejection>, data: &Data) -> Result<String, ()> {
    let Query(params) = query.map_err(|_| ())?;

    let osu = Osu::builder()
        .client_id(data.web.osu_client_id)
        .client_secret(data.web.osu_client_secret.expose_secret())
        .with_authorization(
            params.code,
            "https://verify.osucord.moe",
            rosu_v2::prelude::Scopes::Identify,
        )
        .build()
        .await
        .map_err(|_| ())?;

    let user = osu.own_data().mode(GameMode::Osu).await.map_err(|_| ())?;

    let context = IndexContext {
        success: true,
        user: Some(&user.username),
    };

    let page = data
        .web
        .handlebars
        .render("index", &context)
        .map_err(|_| ())?;

    data.web.auth_standby.process_osu(user, params.state);

    Ok(page)
}

pub struct Metadata {
    key: Key,
    osu_id: u32,
    gamemode: GameMode,
    rank: Option<u32>,
    map_status: UserMapHolder,
    verified_roles: Vec<RoleId>,
    initial_verification: bool,
}

pub async fn task(
    ctx: serenity::all::Context,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<VerificationCommand>,
) {
    let data = ctx.data::<Data>();
    let mut delay_queue = DelayQueue::new();
    let mut keys = HashMap::new();
    let mut empty_fill_instant = std::time::Instant::now();
    let mut interval = tokio::time::interval(Duration::from_secs(1));

    loop {
        tokio::select! {
            Some(cmd) = rx.recv() => {
                handle_verification_command(cmd, &ctx, &data, &mut delay_queue, &mut keys).await;
            },
            Some(expired) = delay_queue.next() => {
                handle_expired_entry(expired, &ctx, &data, &mut keys).await;
            },
            _ = interval.tick() => {
                handle_interval_tick(&data, &mut delay_queue, &mut keys, &mut empty_fill_instant).await;
            }
        }
    }
}

async fn handle_verification_command(
    cmd: VerificationCommand,
    _: &serenity::all::Context,
    data: &Data,
    delay_queue: &mut DelayQueue<UserId>,
    keys: &mut HashMap<UserId, Metadata>,
) {
    match cmd {
        VerificationCommand::Link((u, o, mode)) => {
            let key = delay_queue.insert(u, Duration::from_secs(86400));
            keys.insert(
                u,
                Metadata {
                    key,
                    osu_id: o,
                    gamemode: mode,
                    rank: None,
                    map_status: UserMapHolder::default(),
                    verified_roles: vec![],
                    initial_verification: true,
                },
            );
        }
        VerificationCommand::Unlink((u, _)) => {
            if let Some(metadata) = keys.remove(&u) {
                delay_queue.remove(&metadata.key);
            }
        }
        VerificationCommand::GameModeChange(u, mode) => {
            if let Some(metadata) = keys.get_mut(&u) {
                metadata.initial_verification = true;
                metadata.gamemode = mode;
                metadata.rank = None;
                delay_queue.reset_at(&metadata.key, tokio::time::Instant::now());
            } else {
                let Ok(user) = sqlx::query!(
                    r#"
                        SELECT
                            u.user_id,
                            vu.osu_id,
                            vu.last_updated,
                            vu.rank,
                            vu.map_status,
                            vu.verified_roles
                        FROM
                            verified_users vu
                        JOIN
                            users u ON vu.user_id = u.id
                        WHERE
                            vu.is_active = TRUE AND vu.user_id = $1
                    "#,
                    match data.database.get_user(u).await {
                        Ok(u) => u.id, // internal DB ID (users.id)
                        Err(_) => return,
                    }
                )
                .fetch_one(&data.database.db)
                .await
                else {
                    return;
                };

                let map_status = user
                    .map_status
                    .map(|bits: i16| UserMapHolder::from_bits(bits as u8))
                    .unwrap_or_default();

                let verified_roles = user
                    .verified_roles
                    .unwrap_or_default()
                    .into_iter()
                    .map(|r| RoleId::new(r as u64))
                    .collect();

                let key = delay_queue.insert(u, Duration::from_secs(2));
                keys.insert(
                    u,
                    Metadata {
                        key,
                        osu_id: user.osu_id as u32,
                        gamemode: mode,
                        rank: user.rank.map(|r| r as u32),
                        map_status,
                        verified_roles,
                        initial_verification: true,
                    },
                );
            }
        }
        VerificationCommand::Shutdown => std::process::exit(0),
    }
}

async fn handle_expired_entry(
    expired: Expired<UserId>,
    ctx: &serenity::all::Context,
    data: &Data,
    keys: &mut HashMap<UserId, Metadata>,
) {
    let u = expired.into_inner();
    if let Some(metadata) = keys.remove(&u) {
        let osu = &data.web.osu;
        let valid = match osu.user(metadata.osu_id).mode(metadata.gamemode).await {
            Ok(osu_user) => {
                maybe_update(ctx, u, Some(&osu_user), Some(MetadataType::Full(&metadata))).await
            }
            Err(e) => {
                dbg!(e);
                false
            }
        };

        if !valid {
            let mentions = serenity::all::CreateAllowedMentions::new()
                .all_users(false)
                .everyone(false)
                .all_roles(false);

            let _ = LOG_CHANNEL.send_message(
                &ctx.http,
                CreateMessage::new()
                    .content(format!(
                        "❌ Could not update <@{u}>'s roles due to error: (https://osu.ppy.sh/users/{})",
                        metadata.osu_id
                    ))
                    .allowed_mentions(mentions),
            ).await;

            let _ = data.database.inactive_user(u).await;
        }
    }
}

async fn handle_interval_tick(
    data: &Data,
    delay_queue: &mut DelayQueue<UserId>,
    keys: &mut HashMap<UserId, Metadata>,
    empty_fill_instant: &mut Instant,
) {
    if delay_queue.is_empty() && empty_fill_instant.elapsed() > Duration::from_secs(30) {
        let Ok(users) = sqlx::query!(
            r#"
            SELECT
                u.user_id,
                vu.osu_id,
                vu.last_updated,
                vu.rank,
                vu.gamemode,
                vu.map_status,
                vu.verified_roles
            FROM
                verified_users vu
            JOIN
                users u ON vu.user_id = u.id
            WHERE
                vu.is_active = TRUE
            ORDER BY
                vu.last_updated ASC
            LIMIT 100
            "#
        )
        .fetch_all(&data.database.db)
        .await
        else {
            return;
        };

        let now = Utc::now();

        for user in users {
            let target_time = user.last_updated + chrono::Duration::days(1);
            let seconds = (target_time - now).num_seconds().max(0) as u64;

            let map_status = user
                .map_status
                .map(|bits| UserMapHolder::from_bits(bits as u8))
                .unwrap_or_default();

            let verified_roles = user
                .verified_roles
                .unwrap_or_default()
                .into_iter()
                .map(|r| RoleId::new(r as u64))
                .collect();

            let key =
                delay_queue.insert((user.user_id as u64).into(), Duration::from_secs(seconds));

            keys.insert(
                (user.user_id as u64).into(),
                Metadata {
                    key,
                    osu_id: user.osu_id as u32,
                    gamemode: (user.gamemode as u8).into(),
                    rank: user.rank.map(|r| r as u32),
                    initial_verification: false,
                    map_status,
                    verified_roles,
                },
            );
        }

        *empty_fill_instant = Instant::now();
    }
}
