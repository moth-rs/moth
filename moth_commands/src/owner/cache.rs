use crate::{owner::admin, Context, Error};
use futures::StreamExt;

use ::serenity::all::{collect, Event, GuildId};
use lumi::serenity_prelude as serenity;
use std::{fmt::Write, time::Duration};

/// View/set max messages cached per channel.
#[lumi::command(
    rename = "max-messages",
    prefix_command,
    category = "Admin - Cache",
    check = "admin",
    hide_in_help
)]
pub async fn max_messages(
    ctx: Context<'_>,
    #[description = "Set this value to change the cache limit."] value: Option<u16>,
) -> Result<(), Error> {
    if let Some(val) = value {
        ctx.say(format!(
            "Max messages cached per channel set: **{}** -> **{}**",
            ctx.cache().settings().max_messages,
            val
        ))
        .await?;
        ctx.cache().set_max_messages(val.into());
    } else {
        ctx.say(format!(
            "Max messages cached per channel is set to: **{}**",
            ctx.cache().settings().max_messages
        ))
        .await?;
    }
    Ok(())
}

#[lumi::command(
    rename = "guild-cache-stats",
    prefix_command,
    category = "Admin - Cache",
    hide_in_help,
    check = "admin",
    guild_only
)]
pub async fn guild_cache_stats(ctx: Context<'_>) -> Result<(), Error> {
    let (channel_count, thread_count, members_count, cached_members) = {
        let guild = ctx.guild().unwrap();

        let channel_count = guild.channels.len();
        let thread_count = guild.threads.len();
        let members_count = guild.member_count;
        let cached_members = guild.members.len();

        (channel_count, thread_count, members_count, cached_members)
    };

    let stats = format!(
        "Channel Count: {channel_count}\n Thread count: {thread_count}\nUser count: \
         {members_count}\nCached Users: {cached_members}"
    );

    let embed = serenity::CreateEmbed::default()
        .title("Guild Cache Stats")
        .field("Stats", stats, true);

    ctx.send(lumi::CreateReply::default().embed(embed)).await?;

    Ok(())
}

#[lumi::command(
    rename = "guild-user-cache",
    prefix_command,
    category = "Admin - Cache",
    hide_in_help,
    check = "admin"
)]
pub async fn guild_user_cache(
    ctx: Context<'_>,
    chunk: Option<bool>,
    guild_id: Option<GuildId>,
) -> Result<(), Error> {
    let Some(guild_id) = guild_id.or_else(|| ctx.guild_id()) else {
        ctx.say("You are not in a guild and you didn't specify a GuildId.")
            .await?;
        return Ok(());
    };

    let successful_chunk = if chunk.unwrap_or(false) {
        let chunked = Some(chunk_and_wait(ctx, guild_id).await);
        // we can actually do a cache access before its finished writing.
        tokio::time::sleep(Duration::from_secs(1)).await;

        chunked
    } else {
        None
    };

    let mut user_str = String::new();
    let guild_name = {
        let Some(cache) = ctx.cache().guild(guild_id) else {
            ctx.say("Guild was not found in cache.").await?;
            return Ok(());
        };

        cache.members.iter().for_each(|m| {
            writeln!(
                user_str,
                "{}: {}: {:?}, {:?}",
                m.user.id.get(),
                m.user.name,
                m.user.global_name,
                m.nick
            )
            .unwrap();
        });
        cache.name.clone()
    };

    let mut content = String::new();
    if !successful_chunk.unwrap_or(true) {
        writeln!(
            content,
            "Could not chunk guild successfully, member count may be innaccurate."
        )
        .unwrap();
    }

    write!(
        content,
        "**{}** Members in cache for {guild_name}",
        user_str.lines().count()
    )
    .unwrap();

    let mentions = serenity::CreateAllowedMentions::new()
        .all_roles(false)
        .all_users(false)
        .everyone(false);

    ctx.send(
        lumi::CreateReply::new()
            .content(content)
            .allowed_mentions(mentions)
            .attachment(serenity::CreateAttachment::bytes(
                user_str.into_bytes(),
                "users.txt",
            )),
    )
    .await?;

    Ok(())
}

/// Chunks the guild members and waits until completion.
///
/// Returns `false` if it does not finish fast enough.
async fn chunk_and_wait(ctx: Context<'_>, guild_id: GuildId) -> bool {
    ctx.serenity_context().chunk_guild(
        guild_id,
        None,
        false,
        serenity::ChunkGuildFilter::None,
        Some(ctx.id().to_string()),
    );

    let mut stream = collect(ctx.serenity_context(), |event| match event {
        Event::GuildMembersChunk(e) => Some((e.nonce.clone(), e.chunk_count, e.chunk_index)),
        _ => None,
    });

    println!("spawned collector for chunks.");

    // each iteration has this much time.
    let timeout_duration = Duration::from_secs(2);
    // this is the total time allowed.
    let total_timeout = Duration::from_secs(15);
    let start_time = tokio::time::Instant::now();

    while let Ok(Some((nonce, count, index))) =
        tokio::time::timeout(timeout_duration, stream.next()).await
    {
        if start_time.elapsed() > total_timeout {
            return false; // Total timeout hit.
        }

        if let Some(nonce) = nonce {
            if nonce.as_str() == ctx.id().to_string().as_str() && index + 1 == count {
                return true;
            }
        }
    }

    false
}

#[must_use]
pub fn commands() -> [crate::Command; 3] {
    [max_messages(), guild_cache_stats(), guild_user_cache()]
}
