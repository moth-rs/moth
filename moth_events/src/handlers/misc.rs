use crate::{Data, Error};
use lumi::serenity_prelude::{self as serenity, Ready};

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

pub async fn ready(ctx: &serenity::Context, ready: &Ready, data: Arc<Data>) -> Result<(), Error> {
    let activity_data = serenity::ActivityData {
        name: small_fixed_array::FixedString::from_str_trunc("Banging myself against your window."),
        kind: serenity::ActivityType::Custom,
        state: None,
        url: None,
    };
    ctx.set_activity(Some(activity_data));

    let shard_count = ctx.cache.shard_count();
    let is_last_shard = (ctx.shard_id.0 + 1) == shard_count.get();

    if is_last_shard && !data.has_started.swap(true, Ordering::SeqCst) {
        finalize_start(ctx).await;
        println!("Logged in as {}", ready.user.tag());
    }

    Ok(())
}

async fn finalize_start(ctx: &serenity::Context) {
    let data = ctx.data::<Data>();
    let data_clone = data.clone();

    tokio::spawn(async move {
        let mut interval: tokio::time::Interval = tokio::time::interval(Duration::from_secs(2));
        loop {
            interval.tick().await;
            data_clone.anti_delete_cache.decay_proc();
        }
    });

    let data_clone = data.clone();
    tokio::spawn(moth_core::verification::run(data_clone));
    data.web.start_background_task(ctx.clone()).await;
}
