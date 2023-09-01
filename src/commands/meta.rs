use std::{fs::File, io::Read, time::Instant};

use poise::serenity_prelude as serenity;
use poise::serenity_prelude::ChannelId;
use toml::Value;

use crate::{Context, Error};

#[poise::command(prefix_command, owners_only, hide_in_help)]
pub async fn shutdown(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("**Bailing out, you are on your own. Good luck.**")
        .await?;
    ctx.framework()
        .shard_manager()
        .lock()
        .await
        .shutdown_all()
        .await;
    Ok(())
}

#[poise::command(slash_command, prefix_command, category = "Meta")]
pub async fn uptime(ctx: Context<'_>) -> Result<(), Error> {
    let uptime = std::time::Instant::now() - ctx.data().time_started;

    let calculation = |a, b| (a / b, a % b);

    let seconds = uptime.as_secs();
    let (minutes, seconds) = calculation(seconds, 60);
    let (hours, minutes) = calculation(minutes, 60);
    let (days, hours) = calculation(hours, 24);

    ctx.say(format!(
        "`Uptime: {}d {}h {}m {}s`",
        days, hours, minutes, seconds
    ))
    .await?;

    Ok(())
}

// Post a link to my source code!
#[poise::command(slash_command, prefix_command, category = "Meta")]
pub async fn source(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("<https://github.com/jamesbt365/jamespy-rs>\n<https://github.com/jamesbt365/jamespy/tree/frontend>").await?;
    Ok(())
}

// About jamespy!
#[poise::command(slash_command, prefix_command, category = "Meta")]
pub async fn about(ctx: Context<'_>) -> Result<(), Error> {
    let version = {
        let mut file = File::open("Cargo.toml")?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let data = contents.parse::<Value>().unwrap();
        let version = data["package"]["version"].as_str().unwrap();
        version.to_string()
    };

    let uptime = std::time::Instant::now() - ctx.data().time_started;
    let calculation = |a, b| (a / b, a % b);

    let seconds = uptime.as_secs();
    let (minutes, _seconds) = calculation(seconds, 60);
    let (hours, minutes) = calculation(minutes, 60);
    let (days, hours) = calculation(hours, 24);
    let uptime_string = format!("{}d{}h{}m", days, hours, minutes);


    let bot_user = ctx.serenity_context().cache.current_user().clone();
    let bot_name = bot_user.name.clone();
    let bot_avatar = bot_user.avatar_url();

    let guild_num = ctx.serenity_context().cache.guilds().len();
    let channel_num = ctx.serenity_context().cache.guild_channel_count();
    let user_num = ctx.serenity_context().cache.user_count();

    let mut embed = serenity::CreateEmbed::default()
        .title(format!("**{} - v{}**", bot_name, version))
        .description("A general spy bot that only exists to spy! It has no other purpose.")
        .field("Stats:", format!("Guilds: {}\n Channels: {}\n Users: {}", guild_num, channel_num, user_num), true)
        .field("Usage stats:", format!("Uptime:\n `{}`", uptime_string), true)
        .field("Memory stats:", "Not implemented", true);
        // Add footer

    if let Some(avatar_url) = bot_avatar {
        embed = embed.thumbnail(avatar_url);
    }

    let msg = poise::CreateReply::default().embed(embed);


    ctx.send(msg).await?;
    Ok(())
}

/// Show general help or help to a specific command
#[poise::command(prefix_command, track_edits, slash_command, category = "Miscellaneous")]
pub async fn help(
    ctx: Context<'_>,
    #[description = "Specific command to show help about"]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    command: Option<String>,
) -> Result<(), Error> {
    poise::builtins::help(
        ctx,
        command.as_deref(),
        poise::builtins::HelpConfiguration {
            ephemeral: true,
            ..Default::default()
        },
    )
    .await?;
    Ok(())
}

/// Say something!
#[poise::command(prefix_command, hide_in_help, owners_only)]
pub async fn say(
    ctx: Context<'_>,
    #[description = "Channel where the message will be sent"] channel: Option<ChannelId>,
    #[description = "What to say"] string: String,
) -> Result<(), Error> {
    let target_channel = channel.unwrap_or(ctx.channel_id());

    target_channel.say(&ctx.http(), string).await?;

    Ok(())
}

/// pong!
#[poise::command(slash_command, prefix_command, category = "Meta", user_cooldown = 10)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    let now = Instant::now();
    reqwest::get("https://discordapp.com/api/v6/gateway").await?;
    let get_latency = now.elapsed().as_millis();

    let now = Instant::now();
    let ping_msg = ctx.say("Calculating...").await?;
    let post_latency = now.elapsed().as_millis();

    ping_msg
        .edit(
            ctx,
            poise::CreateReply::default().content("").embed(
                serenity::CreateEmbed::default()
                    .title("Pong!")
                    .field("GET Latency", format!("{}ms", get_latency), false)
                    .field("POST Latency", format!("{}ms", post_latency), false),
            ),
        )
        .await?;

    Ok(())
}
