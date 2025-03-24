use crate::{Context, Error};
use lumi::serenity_prelude::{
    self as serenity, ActivityType, GuildMemberFlags, OnlineStatus, User,
};
use std::collections::HashMap;
use std::fmt::Write;

#[lumi::command(
    slash_command,
    prefix_command,
    category = "Utility",
    guild_only,
    user_cooldown = 15
)]
pub async fn statuses(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let cache = &ctx.cache();
    let guild = cache.guild(guild_id).unwrap().clone(); // I don't know how to use new stuff.

    let mut status_counts = HashMap::new();
    let mut message = String::new();
    for presence in &guild.presences {
        let status = presence.status;

        let count = status_counts.entry(status).or_insert(0);
        *count += 1;
    }

    for (status, count) in &status_counts {
        let status_message = match status {
            OnlineStatus::DoNotDisturb => format!("Do Not Disturb: {count}"),
            OnlineStatus::Idle => format!("Idle: {count}"),
            OnlineStatus::Invisible => format!("Invisible: {count}"),
            OnlineStatus::Offline => format!("Offline: {count}"),
            OnlineStatus::Online => format!("Online: {count}"),
            _ => String::new(),
        };

        message.push_str(&status_message);
        message.push('\n');
    }
    message.push_str(&guild.presences.len().to_string());
    ctx.send(lumi::CreateReply::default().content(message))
        .await?;

    Ok(())
}

/// See what games people are playing!
#[lumi::command(
    slash_command,
    prefix_command,
    category = "Utility",
    guild_only,
    user_cooldown = 15
)]
pub async fn playing(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let cache = &ctx.cache();
    // i really should try and avoid this clone.
    let guild = cache.guild(guild_id).unwrap().clone();

    let total_members = guild
        .presences
        .iter()
        .filter(|presence| {
            presence
                .activities
                .iter()
                .any(|activity| activity.kind == ActivityType::Playing)
        })
        .count();

    let mut activity_counts: HashMap<&str, u32> = HashMap::new();
    for presence in &guild.presences {
        for activity in &presence.activities {
            if activity.kind == ActivityType::Playing {
                let name = &activity.name;
                let count = activity_counts.entry(name.as_str()).or_insert(0);
                *count += 1;
            }
        }
    }

    let total_games: usize = activity_counts.values().len();

    let mut vec: Vec<(&&str, &u32)> = activity_counts.iter().collect();
    vec.sort_by(|a, b| b.1.cmp(a.1));

    let pages: Vec<Vec<(&str, u32)>> = vec
        .iter()
        .map(|&(name, count)| (*name, *count))
        .collect::<Vec<(&str, u32)>>()
        .chunks(15)
        .map(<[(&str, u32)]>::to_vec)
        .collect();

    crate::utils::presence_builder(ctx, pages, total_members, total_games).await?;

    Ok(())
}

/// See what osu! gamemodes people are playing!
#[lumi::command(
    slash_command,
    prefix_command,
    category = "Utility",
    guild_only,
    user_cooldown = 5
)]
pub async fn osu(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let cache = &ctx.cache();
    // i really should try and avoid this clone.
    let guild = cache.guild(guild_id).unwrap().clone();

    let mut total = 0;
    let mut osu = 0;
    let mut mania = 0;
    let mut taiko = 0;
    let mut catch = 0;
    let mut other = 0;
    let mut unknown = 0;

    for presence in &guild.presences {
        for activity in &presence.activities {
            if activity.application_id == Some(367827983903490050.into()) {
                total += 1;

                if let Some(assets) = &activity.assets {
                    match assets.small_text.as_deref() {
                        Some("osu!") => osu += 1,
                        Some("osu!mania") => mania += 1,
                        Some("osu!taiko") => taiko += 1,
                        Some("osu!catch") => catch += 1,
                        _ => other += 1,
                    }
                } else {
                    unknown += 1;
                }
            }
        }
    }

    let description = if other > 0 {
        format!(
            "Total: {total}\nosu!: {osu}\nmania: {mania}\ntaiko: {taiko}\ncatch: {catch}\nother: \
             {other}\nunknown: {unknown}"
        )
    } else {
        format!(
            "Total: {total}\nosu!: {osu}\nmania: {mania}\ntaiko: {taiko}\ncatch: \
             {catch}\nunknown: {unknown}"
        )
    };

    let embed = serenity::CreateEmbed::new()
        .title("osu! gamemode popularity")
        .description(description)
        .colour(serenity::Colour::BLUE);

    ctx.send(lumi::CreateReply::new().embed(embed)).await?;

    Ok(())
}

/// See information about a users dm activity flag.
#[lumi::command(
    rename = "dm-activity-check",
    aliases("dm-activity"),
    prefix_command,
    category = "Utility",
    guild_only,
    required_permissions = "MANAGE_MESSAGES"
)]
pub async fn dm_activity_check(ctx: Context<'_>, user: User) -> Result<(), Error> {
    if ctx.guild_id().unwrap() != 98226572468690944 {
        return Ok(());
    }

    let author =
        serenity::CreateEmbedAuthor::new(format!("{}'s unusual dm activity info", user.tag()))
            .icon_url(user.avatar_url().unwrap_or_default());

    let mut embed = serenity::CreateEmbed::default().author(author);

    let result = ctx.data().get_activity_check(user.id).await;

    if let Some(result) = result {
        let until = if let Some(u) = result.until {
            format!("<t:{u}>")
        } else {
            String::from("None")
        };

        embed = embed
            .field(
                "Announced last",
                format!("<t:{}>", result.last_announced),
                true,
            )
            .field("Until", until, true)
            .field("Count", result.count.to_string(), true);
    }

    if let Ok(member) = ctx.guild_id().unwrap().member(ctx, user.id).await {
        let until = if let Some(activity) = member.unusual_dm_activity_until {
            format!("<t:{}>", activity.unix_timestamp())
        } else {
            String::from("None")
        };
        embed = embed.field("Currently flagged until?", until, false);
    }

    ctx.send(lumi::CreateReply::default().embed(embed)).await?;

    Ok(())
}

/// See what games people are playing!
#[lumi::command(
    rename = "flag-lb",
    aliases("flagged-lb", "dm-activity-lb"),
    prefix_command,
    category = "Utility",
    guild_only,
    required_permissions = "MANAGE_MESSAGES"
)]
pub async fn flag_lb(ctx: Context<'_>) -> Result<(), Error> {
    if ctx.guild_id().unwrap() != 98226572468690944 {
        return Ok(());
    }

    // lumi will display an error if this goes wrong, though at the same time it'll show an error if nobody is on the list.
    let result =
        sqlx::query!("SELECT user_id, count FROM dm_activity ORDER BY count DESC LIMIT 20")
            .fetch_all(&ctx.data().database.db)
            .await
            .unwrap();

    let mut description = String::new();
    for (index, record) in result.into_iter().enumerate() {
        writeln!(
            description,
            "**{}**. <@{}>: **{}**",
            index + 1,
            record.user_id,
            record.count.unwrap()
        )
        .unwrap();
    }

    let embed = serenity::CreateEmbed::default()
        .title("Top 20 users flagged with dm-activity")
        .description(description);

    ctx.send(lumi::CreateReply::default().embed(embed)).await?;

    Ok(())
}

/// Display some details from the member object.
#[lumi::command(
    prefix_command,
    category = "Utility",
    guild_only,
    required_permissions = "MANAGE_MESSAGES"
)]
pub async fn presence(ctx: Context<'_>, member: serenity::Member) -> Result<(), Error> {
    let data = {
        let guild = ctx.guild().unwrap();
        guild.presences.get(&member.user.id).cloned()
    };

    ctx.say(format!("{data:?}")).await?;

    Ok(())
}

/// Display some details from the member object.
#[lumi::command(
    rename = "get-member",
    prefix_command,
    category = "Utility",
    guild_only,
    required_permissions = "MANAGE_MESSAGES",
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
pub async fn get_member(ctx: Context<'_>, member: serenity::Member) -> Result<(), Error> {
    let mut embed = serenity::CreateEmbed::default();

    embed = embed.title(format!("{}'s Member Object", &member.user.tag()));

    if let Some(avatar) = member.avatar_url() {
        embed = embed.thumbnail(avatar);
    }

    if let Some(nick) = member.nick.clone() {
        embed = embed.field("Nickname", nick, true);
    }

    if let Some(joined_at) = member.joined_at {
        embed = embed.field("Joined at", joined_at.to_string(), true);
    }

    if let Some(boosting) = member.premium_since {
        embed = embed.field("Boosting since", boosting.to_string(), true);
    }

    if let Some(flags) = get_flags_str(member.flags) {
        embed = embed.field("Flags", flags, true);
    }

    if let Some(comms_disabled) = member.communication_disabled_until {
        embed = embed.field("Timeout until", comms_disabled.to_string(), true);
    }

    if let Some(dm_activity) = member.unusual_dm_activity_until {
        embed = embed.field("High DM Activity Until", dm_activity.to_string(), true);
    }

    embed = embed.field("Pending", member.pending().to_string(), true);

    ctx.send(lumi::CreateReply::default().embed(embed)).await?;

    Ok(())
}

#[must_use]
pub fn commands() -> [crate::Command; 7] {
    [
        statuses(),
        playing(),
        dm_activity_check(),
        flag_lb(),
        get_member(),
        osu(),
        presence(),
    ]
}

fn get_flags_str(flags: GuildMemberFlags) -> Option<String> {
    let flag_strings: Vec<&str> = [
        ("DID_REJOIN", GuildMemberFlags::DID_REJOIN),
        (
            "COMPLETED_ONBOARDING",
            GuildMemberFlags::COMPLETED_ONBOARDING,
        ),
        (
            "BYPASSES_VERIFICATION",
            GuildMemberFlags::BYPASSES_VERIFICATION,
        ),
        ("STARTED_ONBOARDING", GuildMemberFlags::STARTED_ONBOARDING),
    ]
    .iter()
    .filter_map(|(name, flag)| {
        if flags.contains(*flag) {
            Some(*name)
        } else {
            None
        }
    })
    .collect();

    if flag_strings.is_empty() {
        None
    } else {
        Some(flag_strings.join("\n"))
    }
}
