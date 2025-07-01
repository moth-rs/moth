use lumi::serenity_prelude as serenity;
use moth_core::emojis::{Checkmark, Question, X};
use small_fixed_array::FixedString;

use crate::{Error, PrefixContext};

#[lumi::command(
    rename = "+",
    prefix_command,
    category = "Admin - Roles",
    owners_only,
    hide_in_help,
    guild_only
)]
pub async fn role_add(
    ctx: PrefixContext<'_>,
    #[lazy] user_id: Option<serenity::Member>,
    #[rest] role_name: FixedString<u8>,
) -> Result<(), Error> {
    let user_id = user_id
        .map(|m| m.user.id)
        .or_else(|| ctx.msg.referenced_message.as_ref().map(|m| m.author.id));
    let Some(user_id) = user_id else {
        ctx.say("Who?").await?;
        return Ok(());
    };

    let role = match get_role(ctx, &role_name) {
        Ok(x) => x,
        Err(e) => {
            ctx.say(e).await?;
            return Ok(());
        }
    };

    let Some(role) = role else {
        let _ = ctx
            .msg
            .react(
                ctx.http(),
                serenity::ReactionType::Unicode(Question::to_fixed_string()),
            )
            .await;
        return Ok(());
    };

    // honestly, this entire thing needs a refactor so i'll just hit the cache again so i don't have to refactor it.
    let higher = ctx.guild().is_none_or(|g| {
        let highest_role = g.member_highest_role(
            g.members
                .get(&ctx.cache().current_user().id)
                .expect("Discord docs indicate the bot user is always in cache"),
        );

        highest_role.is_none_or(|r| r.position > role.1)
    });

    if higher {
        ctx.http()
            .add_member_role(
                ctx.guild_id().unwrap(),
                user_id,
                role.0,
                Some(&format!("Added by {}.", ctx.author().name)),
            )
            .await?;

        let _ = ctx
            .msg
            .react(
                ctx.http(),
                serenity::ReactionType::Unicode(Checkmark::to_fixed_string()),
            )
            .await;
    } else {
        let _ = ctx
            .msg
            .react(
                ctx.http(),
                serenity::ReactionType::Unicode(X::to_fixed_string()),
            )
            .await;
    }

    Ok(())
}

const ERR_ASSIGNABLE: &str = "This is a linked or managed role; it cannot be assigned.";
const ERR_DUPLICATE: &str = "Multiple roles contain this name; use the Id.";

fn get_role(
    ctx: PrefixContext<'_>,
    role_name: &FixedString<u8>,
) -> Result<Option<(serenity::RoleId, i16)>, &'static str> {
    let parsed_role = role_name.parse::<serenity::RoleId>().ok();

    // we just can't check a cache if it doesn't exist.
    let Some(guild) = ctx.guild() else {
        return Ok(parsed_role.map(|id| (id, 0)));
    };

    if let Some(role_id) = parsed_role {
        if let Some(role) = guild.roles.get(&role_id) {
            if is_assignable(role) {
                return Ok(Some((role.id, role.position)));
            }
            return Err(ERR_ASSIGNABLE);
        }
    }

    find_unique_role(role_name, &guild)
}

fn is_assignable(role: &serenity::Role) -> bool {
    !role.managed() && !role.tags.guild_connections()
}

fn find_unique_role(
    role_name: &str,
    guild: &serenity::Guild,
) -> Result<Option<(serenity::RoleId, i16)>, &'static str> {
    let mut found: Option<&serenity::Role> = None;

    for role in &guild.roles {
        if role.name.eq_ignore_ascii_case(role_name) {
            if found.is_some() {
                return Err(ERR_DUPLICATE);
            }
            found = Some(role);
        }
    }

    if let Some(role) = found {
        return if is_assignable(role) {
            Ok(Some((role.id, role.position)))
        } else {
            Err(ERR_ASSIGNABLE)
        };
    }

    for role in &guild.roles {
        if role.name.to_lowercase().contains(&role_name.to_lowercase()) {
            if found.is_some() {
                return Err(ERR_DUPLICATE);
            }
            found = Some(role);
        }
    }

    if let Some(role) = found {
        if is_assignable(role) {
            Ok(Some((role.id, role.position)))
        } else {
            Err(ERR_ASSIGNABLE)
        }
    } else {
        Ok(None)
    }
}

#[lumi::command(
    rename = "-",
    prefix_command,
    category = "Admin - Roles",
    owners_only,
    hide_in_help,
    guild_only
)]
pub async fn role_remove(
    ctx: PrefixContext<'_>,
    #[lazy] user_id: Option<serenity::Member>,
    #[rest] role_name: FixedString<u8>,
) -> Result<(), Error> {
    let user_id = user_id
        .map(|m| m.user.id)
        .or_else(|| ctx.msg.referenced_message.as_ref().map(|m| m.author.id));
    let Some(user_id) = user_id else {
        ctx.say("Who?").await?;
        return Ok(());
    };

    let role = match get_role(ctx, &role_name) {
        Ok(x) => x,
        Err(e) => {
            ctx.say(e).await?;
            return Ok(());
        }
    };

    let Some(role) = role else {
        let _ = ctx
            .msg
            .react(
                ctx.http(),
                serenity::ReactionType::Unicode(Question::to_fixed_string()),
            )
            .await;
        return Ok(());
    };

    // honestly, this entire thing needs a refactor so i'll just hit the cache again so i don't have to refactor it.
    let higher = ctx.guild().is_none_or(|g| {
        let highest_role = g.member_highest_role(
            g.members
                .get(&ctx.cache().current_user().id)
                .expect("Discord docs indicate the bot user is always in cache"),
        );

        highest_role.is_none_or(|r| r.position > role.1)
    });

    if higher {
        ctx.http()
            .remove_member_role(
                ctx.guild_id().unwrap(),
                user_id,
                role.0,
                Some(&format!("Removed by {}.", ctx.author().name)),
            )
            .await?;

        let _ = ctx
            .msg
            .react(
                ctx.http(),
                serenity::ReactionType::Unicode(Checkmark::to_fixed_string()),
            )
            .await;
    } else {
        let _ = ctx
            .msg
            .react(
                ctx.http(),
                serenity::ReactionType::Unicode(X::to_fixed_string()),
            )
            .await;
    }

    Ok(())
}

#[must_use]
pub fn commands() -> [crate::Command; 2] {
    [role_add(), role_remove()]
}
