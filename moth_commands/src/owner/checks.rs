use crate::{Context, Error};
use std::fmt::Write;

use crate::utils::{get_cmd_name, handle_allow_cmd, handle_deny_cmd, CommandRestrictErr};
use ::serenity::all::CreateAllowedMentions;
use lumi::{
    serenity_prelude::{self as serenity, User},
    CreateReply,
};

// This entire module needs new command/function names.

#[lumi::command(
    rename = "bot-ban",
    aliases("bb", "bban"),
    prefix_command,
    hide_in_help,
    category = "Owner - Ban",
    owners_only
)]
pub async fn bot_ban(ctx: Context<'_>, user: User) -> Result<(), Error> {
    let inserted = ctx.data().database.set_banned(user.id, true).await?;

    let msg = if inserted {
        format!("{} is already banned from using moth.", user.tag())
    } else {
        format!("{} has been banned from using moth.", user.tag())
    };

    ctx.say(msg).await?;

    Ok(())
}

#[lumi::command(
    rename = "bot-unban",
    aliases("bub", "bunban"),
    prefix_command,
    hide_in_help,
    category = "Owner - Ban",
    owners_only
)]
pub async fn bot_unban(ctx: Context<'_>, user: User) -> Result<(), Error> {
    let banned = ctx.data().database.set_banned(user.id, false).await?;

    let msg = if banned {
        format!("{} has been unbanned from using moth.", user.tag())
    } else {
        format!("{} was not banned from using moth.", user.tag())
    };

    ctx.say(msg).await?;

    Ok(())
}

#[lumi::command(
    rename = "allow-owner-cmd",
    aliases("aoc"),
    prefix_command,
    hide_in_help,
    category = "Owner - Overrides",
    owners_only
)]
#[allow(clippy::match_wildcard_for_single_variants)]
pub async fn allow_owner_cmd(ctx: Context<'_>, user: User, cmd_name: String) -> Result<(), Error> {
    let statement = match handle_allow_cmd(
        &ctx.framework().options.commands,
        &ctx.data(),
        cmd_name,
        &user,
    )
    .await
    {
        Ok(cmd) => format!("Successfully allowed {user} to use `{cmd}`!"),
        Err(err) => match err {
            CommandRestrictErr::CommandNotFound => "Could not find command!",
            CommandRestrictErr::AlreadyExists => "The user is already allowed to use this!",
            CommandRestrictErr::FrameworkOwner => {
                "This command requires you to be an owner in the framework!"
            }
            CommandRestrictErr::NotOwnerCommand => "This command is not an owner command!",
            _ => "",
        }
        .to_string(),
    };

    ctx.send(
        CreateReply::new()
            .content(statement)
            .allowed_mentions(CreateAllowedMentions::new()),
    )
    .await?;

    Ok(())
}

#[lumi::command(
    rename = "deny-owner-cmd",
    aliases("doc"),
    prefix_command,
    category = "Owner - Overrides",
    hide_in_help,
    owners_only
)]
#[allow(clippy::match_wildcard_for_single_variants)]
pub async fn deny_owner_cmd(ctx: Context<'_>, user: User, cmd_name: String) -> Result<(), Error> {
    let statement = match handle_deny_cmd(
        &ctx.framework().options.commands,
        &ctx.data(),
        &cmd_name,
        &user,
    )
    .await
    {
        Ok(cmd) => format!("Successfully denied {user} to use `{cmd}`!"),
        Err(err) => match err {
            CommandRestrictErr::CommandNotFound => "Could not find command!",
            CommandRestrictErr::FrameworkOwner => {
                "This command requires you to be an owner in the framework!"
            }
            CommandRestrictErr::NotOwnerCommand => "This command is not an owner command!",
            CommandRestrictErr::DoesntExist => "Cannot remove permissions they don't have!",
            _ => "", // No other errors should fire in this code.
        }
        .to_string(),
    };

    ctx.send(
        CreateReply::new()
            .content(statement)
            .allowed_mentions(CreateAllowedMentions::new()),
    )
    .await?;

    Ok(())
}

#[lumi::command(
    rename = "owner-overrides",
    aliases("ownero"),
    prefix_command,
    hide_in_help,
    category = "Owner - Overrides",
    owners_only,
    subcommands("user", "cmd"),
    subcommand_required
)]
pub async fn owner_overrides(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[lumi::command(prefix_command, hide_in_help, owners_only)]
pub async fn user(ctx: Context<'_>, user: User) -> Result<(), Error> {
    let overrides = {
        let data = ctx.data();
        let checks = data.database.inner_overwrites();

        let mut single_overrides = Vec::new();
        for single_check in &checks.owners_single {
            if single_check.value().contains(&user.id) {
                single_overrides.push(single_check.key().clone());
            }
        }

        (
            checks.owners_all.get(&user.id).map(|entry| *entry),
            Some(single_overrides),
        )
    };

    // TODO: fix this mess, and paginate.
    match overrides {
        (Some(_), Some(single_overrides)) => {
            let embed = if single_overrides.is_empty() {
                None
            } else {
                let mut description = String::new();
                for over in single_overrides {
                    writeln!(description, "**{over}**").unwrap();
                }

                let embed = serenity::CreateEmbed::new()
                    .title("Extra Owner Overrides")
                    .description(description);
                Some(embed)
            };

            if let Some(embed) = embed {
                let msg = lumi::CreateReply::new()
                    .content("This user has overrides for all owner commands!")
                    .embed(embed);
                ctx.send(msg).await?;
            } else {
                let msg = lumi::CreateReply::new()
                    .content("This user has overrides for all owner commands!");
                ctx.send(msg).await?;
            }
        }
        (None, Some(single_overrides)) => {
            let embed = if single_overrides.is_empty() {
                None
            } else {
                let mut description = String::new();
                for over in single_overrides {
                    writeln!(description, "**{over}**").unwrap();
                }

                let embed = serenity::CreateEmbed::new()
                    .title("Owner Overrides")
                    .description(description);
                Some(embed)
            };

            if let Some(embed) = embed {
                let msg = lumi::CreateReply::new().embed(embed);
                ctx.send(msg).await?;
            } else {
                ctx.say("This user doesn't have any overrides!").await?;
            }
        }
        _ => {
            // This should be the only other case that can happen.
            // The vec will always exist (except for when no config) but will just be empty.
            ctx.say("No overrides exist!").await?;
        }
    }

    Ok(())
}

#[lumi::command(prefix_command, hide_in_help, owners_only)]
pub async fn cmd(ctx: Context<'_>, cmd_name: String) -> Result<(), Error> {
    let res = get_cmd_name(&ctx.framework().options.commands, &cmd_name);

    match res {
        Ok(name) => {
            cmd_overrides(ctx, &name).await?;
        }
        Err(err) => {
            let _ = match err {
                CommandRestrictErr::CommandNotFound => ctx.say("No command was found!").await?,
                CommandRestrictErr::FrameworkOwner => {
                    ctx.say("This command requires framework owner!").await?
                }
                _ => ctx.say("Unknown Error type!").await?, // This shouldn't fire.
            };
        }
    }

    Ok(())
}

// TODO: paginate.
pub async fn cmd_overrides(ctx: Context<'_>, cmd_name: &str) -> Result<(), Error> {
    let overrides = ctx
        .data()
        .database
        .inner_overwrites()
        .owners_single
        .get(cmd_name)
        .map(|entry| entry.value().to_owned());

    if let Some(over) = overrides {
        if over.is_empty() {
            ctx.say("No overrrides for this command!").await?;
            return Ok(());
        }

        let mut description = String::new();
        for u in over {
            writeln!(description, "<@{u}>").unwrap();
        }
        let embed = serenity::CreateEmbed::new()
            .title(format!("Overrides for {cmd_name}"))
            .description(description);

        let msg = lumi::CreateReply::new().embed(embed);
        ctx.send(msg).await?;
    }

    Ok(())
}

#[lumi::command(
    aliases("ao"),
    prefix_command,
    category = "Owner - Overrides",
    hide_in_help,
    owners_only
)]
pub async fn allow_owner(ctx: Context<'_>, user: User) -> Result<(), Error> {
    let statement = match handle_allow_owner(ctx, &user).await {
        Ok(()) => format!("Successfully allowed {user} to use owner commands!"),
        Err(err) => match err {
            CommandRestrictErr::AlreadyExists => format!("{user} already has a bypass!"),
            _ => String::from("Error while handling error: Unexpected Error!"),
        },
    };

    ctx.send(
        CreateReply::new()
            .content(statement)
            .allowed_mentions(CreateAllowedMentions::new()),
    )
    .await?;
    Ok(())
}

async fn handle_allow_owner(ctx: Context<'_>, user: &User) -> Result<(), CommandRestrictErr> {
    // TODO: handle errors better
    if ctx
        .data()
        .database
        .set_owner(user.id, None)
        .await
        .map_err(|_| CommandRestrictErr::AlreadyExists)?
    {
        return Err(CommandRestrictErr::AlreadyExists);
    }

    Ok(())
}

#[lumi::command(
    aliases("do"),
    prefix_command,
    category = "Owner - Overrides",
    hide_in_help,
    owners_only
)]
pub async fn deny_owner(ctx: Context<'_>, user: User) -> Result<(), Error> {
    let statement = match handle_deny_owner(ctx, &user).await {
        Ok(()) => format!("Successfully denied {user}'s usage of owner commands!"),
        Err(err) => match err {
            CommandRestrictErr::DoesntExist => format!("{user} doesn't have a bypass!"),
            _ => String::from("Error while handling error: Unexpected Error!"), // No other errors should fire in this code.
        },
    };

    ctx.send(
        CreateReply::new()
            .content(statement)
            .allowed_mentions(CreateAllowedMentions::new()),
    )
    .await?;
    Ok(())
}

async fn handle_deny_owner(ctx: Context<'_>, user: &User) -> Result<(), CommandRestrictErr> {
    if !ctx
        .data()
        .database
        .remove_owner(user.id, None)
        .await
        .map_err(|_| CommandRestrictErr::DoesntExist)?
    {
        return Err(CommandRestrictErr::DoesntExist);
    }

    Ok(())
}

#[must_use]
pub fn commands() -> [crate::Command; 7] {
    [
        allow_owner_cmd(),
        deny_owner_cmd(),
        owner_overrides(),
        allow_owner(),
        deny_owner(),
        bot_ban(),
        bot_unban(),
    ]
}
