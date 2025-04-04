use std::collections::HashSet;
use std::time::Instant;

use ::serenity::all::{
    ChannelId, ChannelType, GuildChannel, Mentionable, PermissionOverwriteType, RoleId, UserId,
};
use lumi::serenity_prelude as serenity;
use moth_core::emojis::{Anger, Checkmark, Question};
use sysinfo::{Pid, System};
use tokio::io::AsyncWriteExt;

use std::fmt::Write;
use std::str::FromStr;

use crate::moderation::msg_or_reaction;
use crate::{Context, Error};

fn uptime_str(seconds: u64) -> String {
    let calculation = |a, b| (a / b, a % b);
    let (minutes, seconds) = calculation(seconds, 60);
    let (hours, minutes) = calculation(minutes, 60);
    let (days, hours) = calculation(hours, 24);

    format!("`{days}d {hours}h {minutes}m {seconds}s`")
}

/// See how long I've been online for!
#[lumi::command(slash_command, prefix_command, category = "Meta", user_cooldown = 3)]
pub async fn uptime(ctx: Context<'_>) -> Result<(), Error> {
    let uptime = ctx.data().time_started.elapsed().as_secs();

    let uptime_str = uptime_str(uptime);

    ctx.say(uptime_str).await?;

    Ok(())
}

// Post a link to my source code!
#[lumi::command(slash_command, prefix_command, category = "Meta", user_cooldown = 3)]
pub async fn source(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("<https://github.com/moth-rs/moth>").await?;
    Ok(())
}

/// pong!
#[lumi::command(
    slash_command,
    prefix_command,
    category = "Meta",
    user_cooldown = 10,
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    let shard_latency = ctx.serenity_context().runner_info.lock().unwrap().latency;

    // right now i don't have the patience to drop the allocations here where they could be avoided.
    // i'll wait until a macro exists to do this for me.

    let shard = if let Some(latency) = shard_latency {
        ("Shard Latency", format!("{}ms", latency.as_millis()), false)
    } else {
        ("Shard Latency", "Not available".to_string(), false)
    };

    let now = Instant::now();

    ctx.data()
        .reqwest
        .get("https://discordapp.com/api/v6/gateway")
        .send()
        .await?;

    let get_latency = now.elapsed().as_millis();
    let now = Instant::now();
    let ping_msg = ctx.say("Calculating...").await?;
    let post_latency = now.elapsed().as_millis();

    ping_msg
        .edit(
            ctx,
            lumi::CreateReply::default().content("").embed(
                serenity::CreateEmbed::default().title("Pong!").fields([
                    shard,
                    ("GET Latency", format!("{get_latency}ms"), false),
                    ("POST Latency", format!("{post_latency}ms"), false),
                ]),
            ),
        )
        .await?;

    Ok(())
}

fn bytes_to_gibibytes(bytes: u64) -> f64 {
    const GIBIBYTE: f64 = 1024.0 * 1024.0 * 1024.0;
    bytes as f64 / GIBIBYTE
}

fn bytes_to_mebibytes(bytes: u64) -> f64 {
    const MEBIBYTE: f64 = 1024.0 * 1024.0;
    bytes as f64 / MEBIBYTE
}

#[lumi::command(
    prefix_command,
    slash_command,
    hide_in_help,
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
async fn stats(ctx: Context<'_>) -> Result<(), Error> {
    let pid = std::process::id();

    let s = System::new_all();

    let threads = s.cpus().len();
    let total_mem = bytes_to_gibibytes(s.total_memory());
    let used = bytes_to_gibibytes(s.used_memory());

    let seconds = System::uptime();

    let sys_uptime = uptime_str(seconds);
    let bot_uptime = uptime_str(ctx.data().time_started.elapsed().as_secs());

    let mut embed = serenity::CreateEmbed::new()
        .title("Bot & System Statistics")
        .thumbnail(ctx.cache().current_user().face());

    // TODO: readd guild channel count by iterating.
    embed = embed.field(
        "Bot Info",
        format!(
            "Up: {}\nShards: **{}**\nGuilds: **{}**",
            bot_uptime,
            ctx.cache().shard_count(),
            ctx.cache().guilds().len(),
        ),
        true,
    );

    embed = embed.field(
        "System Info",
        format!(
            "CPU Threads: **{threads}**\nMemory: **{used:.2}/{total_mem:.2}** GiB\nUp: \
             {sys_uptime}"
        ),
        true,
    );

    if let Some(process) = s.process(Pid::from(pid as usize)) {
        let physical = bytes_to_mebibytes(process.memory());
        let virtual_m = bytes_to_mebibytes(process.virtual_memory());
        embed = embed.field(
            "Bot Memory",
            format!("Physical: {physical:.2} MiB\nVirtual: {virtual_m:.2} MiB"),
            true,
        );
    }

    ctx.send(lumi::CreateReply::default().embed(embed)).await?;

    Ok(())
}

#[lumi::command(prefix_command, hide_in_help)]
async fn register(ctx: Context<'_>) -> Result<(), Error> {
    // This uses an inbuilt function because spy guild commands should only
    // be registered in the spy guild.
    crate::register::register_application_commands_buttons(ctx).await?;

    Ok(())
}

#[lumi::command(
    aliases("overwrites"),
    prefix_command,
    hide_in_help,
    guild_only,
    required_permissions = "MANAGE_MESSAGES"
)]
async fn overwrite(ctx: Context<'_>, category: Option<GuildChannel>) -> Result<(), Error> {
    let mut count = 0;

    if let Some(category) = &category {
        if category.kind != ChannelType::Category {
            ctx.say("Not a category!").await?;
        }
    }

    if let Some(category) = &category {
        let guild = ctx.guild().unwrap();
        for channel in &guild.channels {
            if let Some(parent) = channel.parent_id {
                if parent == category.id {
                    count += channel.permission_overwrites.len();
                }
            }
        }
    } else {
        let guild = ctx.guild().unwrap();
        for channel in &guild.channels {
            count += channel.permission_overwrites.len();
        }
    }

    if let Some(category) = category {
        ctx.say(format!("{count} overwrites for {category}"))
            .await?;
    } else {
        ctx.say(format!("{count} channel overwrites for the whole server"))
            .await?;
    }

    Ok(())
}

#[derive(Debug, lumi::ChoiceParameter, PartialEq)]
pub enum OverwriteChoices {
    User,
    Role,
}

/// Find permission overwrites for specific users.
#[lumi::command(
    rename = "find-overwrites",
    aliases("find-overwrite"),
    prefix_command,
    hide_in_help,
    guild_only,
    required_permissions = "MANAGE_MESSAGES"
)]
pub async fn find_overwrite(
    ctx: Context<'_>,
    #[description = "Item to find"] choice: OverwriteChoices,
    #[description = "Item to find"] item: String,
) -> Result<(), Error> {
    // manual parsing beacuse no generic ID type exists.
    let Ok(id) = u64::from_str(item.trim_matches(|c| c == '<' || c == '>' || c == '@' || c == '&'))
    else {
        ctx.say("Failure to parse item.").await?;
        return Ok(());
    };

    let overwrite_kind = match choice {
        OverwriteChoices::User => PermissionOverwriteType::Member(UserId::from(id)),
        OverwriteChoices::Role => PermissionOverwriteType::Role(RoleId::from(id)),
    };

    let channel_ids = {
        let guild = ctx.guild().unwrap();

        let channel_ids: Vec<ChannelId> = guild
            .channels
            .iter()
            .filter_map(|channel| {
                channel
                    .permission_overwrites
                    .iter()
                    .find(|overwrite| overwrite.kind == overwrite_kind)
                    .map(|_| channel.id)
            })
            .collect();

        channel_ids
    };

    if channel_ids.is_empty() {
        ctx.say("No permission overwrites exist.").await?;
        return Ok(());
    }

    let mut string = format!("{} total overwrites for ", channel_ids.len());
    if choice == OverwriteChoices::User {
        writeln!(string, "<@{id}>:").unwrap();
    } else {
        writeln!(string, "<@&{id}>:").unwrap();
    }

    let mut description = String::new();
    for channel_id in channel_ids {
        writeln!(description, "<#{channel_id}>").unwrap();
    }

    let mentions = serenity::CreateAllowedMentions::new()
        .all_users(false)
        .everyone(false)
        .all_roles(false);
    ctx.send(
        lumi::CreateReply::new()
            .content(string)
            .embed(
                serenity::CreateEmbed::new()
                    .description(description)
                    .colour(serenity::Colour::BLUE),
            )
            .allowed_mentions(mentions),
    )
    .await?;

    Ok(())
}

use serenity::futures::StreamExt;
use serenity::model::channel::MessagesIter;

/// Find users in a thread to ping.
#[lumi::command(
    prefix_command,
    slash_command,
    hide_in_help,
    guild_only,
    required_permissions = "MANAGE_MESSAGES"
)]
pub async fn scawy(
    ctx: Context<'_>,
    #[channel_types("PublicThread", "PrivateThread")] channel: GuildChannel,
) -> Result<(), Error> {
    if channel.kind != ChannelType::PublicThread && channel.kind != ChannelType::PrivateThread {
        ctx.say("Die.").await?;
        return Ok(());
    }

    ctx.defer().await?;
    let mut users = HashSet::new();
    let mut messages = MessagesIter::stream(ctx.http(), channel.id).boxed();
    while let Some(message_result) = messages.next().await {
        match message_result {
            Ok(message) => {
                println!("Message.");
                if !message.author.bot() {
                    users.insert(message.author.id);
                }
            }
            Err(error) => println!("wtf: {error}"),
        }
    }

    let mut string = String::from("Feel free to paste this whereever: ");
    for user in users {
        write!(string, "{}", user.mention()).unwrap();
    }
    let mentions = serenity::CreateAllowedMentions::new()
        .all_users(false)
        .everyone(false)
        .all_roles(false);

    ctx.send(
        lumi::CreateReply::new()
            .content(string)
            .allowed_mentions(mentions),
    )
    .await?;

    Ok(())
}

// not really meta, but i don't have a misc section.
#[lumi::command(prefix_command, hide_in_help, guild_only, discard_spare_arguments)]
pub async fn template(
    ctx: crate::PrefixContext<'_>,
    file: serenity::Attachment,
) -> Result<(), Error> {
    async fn inner(bytes: Vec<u8>) -> Result<(), Error> {
        if serde_json::from_str::<serde_json::Value>(str::from_utf8(&bytes)?).is_ok() {
            let mut file =
                tokio::fs::File::create("/srv/chibisafe/uploads/mothtemplate.json").await?;
            file.write_all(&bytes).await?;
        } else {
            return Err("Not a valid json file.".into());
        }

        Ok(())
    }

    if !matches!(
        ctx.author().id.get(),
        158567567487795200 | 291089948709486593
    ) {
        msg_or_reaction(ctx, "Nuh uh.", Anger::to_fixed_string()).await;
        return Ok(());
    }

    // a template is not even close to this big.
    if file.size > 1_000_000 {
        msg_or_reaction(ctx, "too big...", Question::to_fixed_string()).await;
        return Ok(());
    }

    let bytes = file.download().await?;

    match inner(bytes).await {
        Ok(()) => msg_or_reaction(ctx, "Done!", Checkmark::to_fixed_string()).await,
        Err(e) => msg_or_reaction(ctx, &e.to_string(), Question::to_fixed_string()).await,
    }

    Ok(())
}

#[must_use]
pub fn commands() -> [crate::Command; 9] {
    [
        uptime(),
        source(),
        ping(),
        register(),
        stats(),
        overwrite(),
        find_overwrite(),
        scawy(),
        template(),
    ]
}
