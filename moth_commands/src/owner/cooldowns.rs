use crate::{Context, Error};
use chrono::{DateTime, NaiveTime, TimeZone, Utc};
use lumi::{
    serenity_prelude::{GenericId, GuildId, UserId},
    CooldownType, StrArg,
};
use serenity::all::GenericChannelId;
use std::time::{Duration, Instant};

#[lumi::command(
    rename = "set-last-invocation",
    aliases("set-invoked"),
    prefix_command,
    category = "Admin - Cooldowns",
    owners_only,
    hide_in_help
)]
pub async fn set_last_invocation(
    ctx: Context<'_>,
    #[description = "Id for the specified bucket."] data: StrArg<GenericId>,
    #[description = "Command by name"] command: String,
    #[description = "The cooldown bucket."] bucket: CooldownBucket,
    #[description = "The time to set to."] time: String,
    #[description = "Allows changing the guild on the Member bucket, use data for the Guild \
                     bucket."]
    guild_id: Option<GuildId>,
) -> Result<(), Error> {
    let commands = &ctx.framework().options.commands;
    let Some(cmd) = commands.iter().find(|cmd| {
        cmd.name == command.to_lowercase()
            || cmd
                .aliases
                .iter()
                .any(|alias| alias == &command.to_lowercase())
    }) else {
        ctx.say("Could not find command!").await?;
        return Ok(());
    };

    let datetime = parse_time(&time)?;
    println!("{datetime}");
    let unix_timestamp = datetime.timestamp();
    // Create a Duration from the Unix timestamp
    let duration_since_epoch = Duration::from_secs(unix_timestamp as u64);
    let target_instant = Instant::now() + duration_since_epoch;

    match bucket {
        CooldownBucket::Global => cmd
            .cooldowns
            .lock()
            .unwrap()
            .set_last_invocation(CooldownType::Global, target_instant),
        CooldownBucket::User => cmd.cooldowns.lock().unwrap().set_last_invocation(
            CooldownType::User(UserId::from(data.0.get())),
            target_instant,
        ),
        CooldownBucket::Guild => {
            cmd.cooldowns.lock().unwrap().set_last_invocation(
                CooldownType::Guild(GuildId::from(data.0.get())),
                target_instant,
            );
        }
        CooldownBucket::Channel => cmd.cooldowns.lock().unwrap().set_last_invocation(
            CooldownType::Channel(GenericChannelId::from(data.0.get())),
            target_instant,
        ),
        CooldownBucket::Member => {
            let target_guild = guild_id.or_else(|| ctx.guild_id());
            let Some(target_guild) = target_guild else {
                ctx.say("Cannot adjust this bucket without specifying a guild.")
                    .await?;
                return Ok(());
            };

            cmd.cooldowns.lock().unwrap().set_last_invocation(
                CooldownType::Member((UserId::from(data.0.get()), target_guild)),
                target_instant,
            );
        }
    }

    ctx.say("Successfully changed last invocation!").await?;

    Ok(())
}

#[must_use]
pub fn commands() -> [crate::Command; 1] {
    [set_last_invocation()]
}

#[derive(Debug, lumi::ChoiceParameter)]
pub enum CooldownBucket {
    Global,
    User,
    Guild,
    Channel,
    Member,
}

fn parse_time(input: &str) -> Result<DateTime<Utc>, Error> {
    // First try to parse the input string as RFC 3339
    if let Ok(datetime_utc) = DateTime::parse_from_rfc3339(input) {
        return Ok(datetime_utc.with_timezone(&Utc));
    }

    // If parsing as RFC 3339 fails, try parsing as time-only format
    let utc_now = Utc::now();
    let current_date = utc_now.date_naive();

    let time_format = if input.contains(':') {
        match input.split(':').collect::<Vec<&str>>().len() {
            2 => "%H:%M",    // HH:MM format
            3 => "%H:%M:%S", // HH:MM:SS format
            _ => return Err("Could not parse time!".into()),
        }
    } else {
        return Err("Could not parse time!".into()); // Invalid format
    };

    let naive_time = NaiveTime::parse_from_str(input, time_format)?;
    let naive_datetime = current_date.and_time(naive_time);

    // Convert NaiveDateTime to DateTime<Utc>
    let datetime_utc = Utc.from_utc_datetime(&naive_datetime);

    Ok(datetime_utc)
}
