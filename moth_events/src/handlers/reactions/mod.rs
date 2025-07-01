use std::sync::Arc;

use crate::helper::{get_channel_name, get_guild_name_override, get_user};
use crate::{Data, Error};

mod database;

use database::*;

use moth_ansi::{HI_MAGENTA, RESET};

use lumi::serenity_prelude::{self as serenity, Reaction};
use small_fixed_array::FixedString;

pub async fn reaction_add(
    ctx: &serenity::Context,
    add_reaction: &Reaction,
    data: Arc<Data>,
) -> Result<(), Error> {
    // I'm not bothered here anymore.
    // will need to to_user when guild_id is none and i'm not adding complexity
    // for reactions that don't matter.
    if add_reaction.guild_id.is_none() {
        return Ok(());
    }

    // recieved over gateway, so a user is present.
    let user_id = add_reaction.user_id.unwrap();
    let user_name = match get_user(ctx, add_reaction.guild_id.unwrap(), user_id).await {
        Some(user) => {
            if user.bot() {
                return Ok(());
            }
            user.tag()
        }
        None => String::from("Unknown User"),
    };

    let guild_id = add_reaction.guild_id;
    let guild_name = get_guild_name_override(ctx, &data, guild_id);

    let channel_name = get_channel_name(ctx, guild_id, add_reaction.channel_id).await;

    println!(
        "{HI_MAGENTA}[{}] [#{}] {} added a reaction: {}{RESET}",
        guild_name, channel_name, user_name, add_reaction.emoji
    );

    let _ = insert_addition(ctx, user_id, add_reaction).await;

    if add_reaction.guild_id == Some(data.starboard_config.guild_id) {
        if let serenity::ReactionType::Unicode(ref unicode) = add_reaction.emoji {
            if unicode == &data.starboard_config.star_emoji {
                moth_starboard::starboard_add_handler(ctx, add_reaction, &data).await?;
            }
        }
    }

    match &add_reaction.emoji {
        serenity::ReactionType::Custom {
            id: _,
            animated: _,
            name,
        } => {
            if name.as_ref() != Some(&FixedString::from_static_trunc("giga_mpreg")) {
                return Ok(());
            }
        }
        _ => return Ok(()),
    }

    let is_james_author = {
        let msgs = ctx.cache.channel_messages(add_reaction.channel_id);
        if let Some(messages) = msgs {
            messages
                .iter()
                .find(|m| m.id == add_reaction.message_id)
                .is_some_and(|m| m.author.id == serenity::UserId::new(158567567487795200))
        } else {
            false
        }
    };

    if is_james_author {
        let _ = add_reaction.delete(&ctx.http).await;
    }

    Ok(())
}

pub async fn reaction_remove(
    ctx: &serenity::Context,
    removed_reaction: &Reaction,
    data: Arc<Data>,
) -> Result<(), Error> {
    // ditto.
    if removed_reaction.guild_id.is_none() {
        return Ok(());
    }

    // ditto.
    let user_id = removed_reaction.user_id.unwrap();
    let user_name = match get_user(ctx, removed_reaction.guild_id.unwrap(), user_id).await {
        Some(user) => {
            if user.bot() {
                return Ok(());
            }
            user.tag()
        }
        None => String::from("Unknown User"),
    };
    let guild_id = removed_reaction.guild_id;
    let guild_name = get_guild_name_override(ctx, &data, guild_id);
    let channel_name = get_channel_name(ctx, guild_id, removed_reaction.channel_id).await;

    println!(
        "{HI_MAGENTA}[{}] [#{}] {} removed a reaction: {}{RESET}",
        guild_name, channel_name, user_name, removed_reaction.emoji
    );

    /*     insert_removal(&data.database, guild_id.unwrap(), user_id, removed_reaction).await?; */

    if removed_reaction.guild_id == Some(data.starboard_config.guild_id) {
        if let serenity::ReactionType::Unicode(ref unicode) = removed_reaction.emoji {
            if unicode == &data.starboard_config.star_emoji {
                moth_starboard::starboard_remove_handler(ctx, removed_reaction, &data).await?;
            }
        }
    }

    Ok(())
}
