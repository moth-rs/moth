use crate::{Context, Error};
use std::{borrow::Cow, fmt};
mod query;
mod utils;

use moth_core::data::database::EmoteUsageType;
use moth_events::handlers::messages::EMOJI_REGEX;
use query::handle_expression_query;

use utils::{check_in_guild, display_expressions};

pub enum Expression<'a> {
    Emote((u64, Cow<'a, str>)),
    Standard(&'a str),
    Id(u64),
    Name(&'a str),
}

impl fmt::Display for Expression<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Expression::Id(id) => write!(f, "{id}"),
            Expression::Name(name) | Expression::Standard(name) => write!(f, "{name}"),
            Expression::Emote((_, name)) => write!(f, "{name}"),
        }
    }
}

#[derive(Debug)]
struct ExpressionCounts {
    user_id: i64,
    reaction_count: Option<i64>,
}

/// Display the usage of emoji's in reactions or messages.
#[lumi::command(
    slash_command,
    prefix_command,
    rename = "emoji-usage",
    category = "Utility",
    guild_only,
    install_context = "Guild",
    interaction_context = "Guild",
    subcommands("reactions", "messages", "all"),
    subcommand_required
)]
pub async fn emoji_usage(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

pub fn string_to_expression(emoji: &str) -> Option<Expression<'_>> {
    let expression = if let Some(capture) = EMOJI_REGEX.captures(emoji) {
        let Ok(id) = capture[3].parse::<u64>() else {
            return None;
        };

        let name_match = capture.get(2).unwrap();

        Expression::Emote((
            id,
            Cow::Borrowed(&emoji[name_match.start()..name_match.end()]),
        ))
    } else if let Ok(emoji_id) = emoji.parse::<u64>() {
        Expression::Id(emoji_id)
    } else if let Some(emoji) = emojis::get(emoji) {
        Expression::Standard(emoji.as_str())
    } else {
        Expression::Name(emoji)
    };

    Some(expression)
}

/// Display usage of a reaction.
#[lumi::command(slash_command, prefix_command, category = "Utility", guild_only)]
pub async fn reactions(ctx: Context<'_>, emoji: String) -> Result<(), Error> {
    let types: [EmoteUsageType; 1] = [EmoteUsageType::Reaction];
    shared(ctx, emoji, &types, Some(false)).await
}

/// Display usage of emoji's through messages.
#[lumi::command(slash_command, prefix_command, category = "Utility", guild_only)]
pub async fn messages(ctx: Context<'_>, emoji: String) -> Result<(), Error> {
    let types = [EmoteUsageType::Message];
    shared(ctx, emoji, &types, Some(true)).await
}

/// Display usage of emojis everywhere.
#[lumi::command(slash_command, prefix_command, category = "Utility", guild_only)]
pub async fn all(ctx: Context<'_>, emoji: String) -> Result<(), Error> {
    let types = [EmoteUsageType::Reaction, EmoteUsageType::Message];
    shared(ctx, emoji, &types, None).await
}

async fn shared(
    ctx: Context<'_>,
    emoji: String,
    types: &[EmoteUsageType],
    msg_type: Option<bool>,
) -> Result<(), Error> {
    if &emoji == "⭐"
        && ctx.guild_id() == Some(98226572468690944.into())
        && types.contains(&EmoteUsageType::Reaction)
    {
        ctx.say("Checking the star reaction usage is disabled to help prevent farming.")
            .await?;
        return Ok(());
    }

    let Some(mut expression) = string_to_expression(&emoji) else {
        ctx.say("I could not parse an expression from this string.")
            .await?;
        return Ok(());
    };

    let in_guild = check_in_guild(ctx, &mut expression).await?;
    if !in_guild {
        ctx.say("You require Manage Messages to be able to check expressions outside the guild.")
            .await?;
        return Ok(());
    }

    let results = handle_expression_query(
        &ctx.data().database,
        &expression,
        ctx.guild_id().unwrap(),
        types,
    )
    .await?;

    display_expressions(ctx, &results, &expression, in_guild, msg_type).await
}

// /emote-leaderboard reactions [duration]
// /emote-leaderboard messages [duration]
// /emote-leaderboard all [duration]

// /sticker-usage [sticker]
// /sticker-leaderboard [duration]

#[must_use]
pub fn commands() -> [crate::Command; 1] {
    [emoji_usage()]
}
