use crate::{Context, Error};

use crate::utils::{handle_allow_cmd, handle_deny_cmd, CommandRestrictErr};
use ::serenity::all::CreateAllowedMentions;
use lumi::{serenity_prelude::User, CreateReply};

// This entire module needs new command/function names.

#[lumi::command(
    rename = "bot-ban",
    aliases("bb", "bban"),
    prefix_command,
    hide_in_help,
    category = "Admin - Ban",
    owners_only
)]
pub async fn bot_ban(ctx: Context<'_>, user: User) -> Result<(), Error> {
    ctx.data().database.set_banned(user.id, true).await?;

    ctx.say(format!(
        "Successfully banned {} from using moth.",
        user.tag()
    ))
    .await?;

    Ok(())
}

#[lumi::command(
    rename = "bot-unban",
    aliases("bub", "bunban"),
    prefix_command,
    hide_in_help,
    category = "Admin - Ban",
    owners_only
)]
pub async fn bot_unban(ctx: Context<'_>, user: User) -> Result<(), Error> {
    ctx.data().database.set_banned(user.id, false).await?;

    ctx.say(format!(
        "Successfully unbanned {} from using moth.",
        user.tag()
    ))
    .await?;

    Ok(())
}

#[lumi::command(
    rename = "allow-admin-cmd",
    aliases("aoc"),
    prefix_command,
    hide_in_help,
    category = "Admin - Overrides",
    owners_only
)]
#[allow(clippy::match_wildcard_for_single_variants)]
pub async fn allow_admin_cmd(ctx: Context<'_>, user: User, cmd_name: String) -> Result<(), Error> {
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
                "This command requires you to be an admin in the framework!"
            }
            CommandRestrictErr::NotOwnerCommand => "This command is not an admin command!",
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
    rename = "deny-admin-cmd",
    aliases("doc"),
    prefix_command,
    category = "Admin - Overrides",
    hide_in_help,
    owners_only
)]
#[allow(clippy::match_wildcard_for_single_variants)]
pub async fn deny_admin_cmd(ctx: Context<'_>, user: User, cmd_name: String) -> Result<(), Error> {
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
                "This command requires you to be an admin in the framework!"
            }
            CommandRestrictErr::NotOwnerCommand => "This command is not an admin command!",
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
    aliases("aa"),
    prefix_command,
    category = "Admin - Overrides",
    hide_in_help,
    owners_only
)]
pub async fn allow_admin(ctx: Context<'_>, user: User) -> Result<(), Error> {
    let statement = match handle_allow_admin(ctx, &user).await {
        Ok(()) => format!("Successfully allowed {user} to use admin commands!"),
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

async fn handle_allow_admin(ctx: Context<'_>, user: &User) -> Result<(), CommandRestrictErr> {
    // TODO: handle errors better
    if ctx
        .data()
        .database
        .set_admin(user.id, None, true)
        .await
        .map_err(|_| CommandRestrictErr::AlreadyExists)?
    {
        return Err(CommandRestrictErr::AlreadyExists);
    }

    Ok(())
}

#[lumi::command(
    aliases("da"),
    prefix_command,
    category = "Admin - Overrides",
    hide_in_help,
    owners_only
)]
pub async fn deny_admin(ctx: Context<'_>, user: User) -> Result<(), Error> {
    let statement = match handle_deny_admin(ctx, &user).await {
        Ok(()) => format!("Successfully denied {user}'s usage of admin commands!"),
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

async fn handle_deny_admin(ctx: Context<'_>, user: &User) -> Result<(), CommandRestrictErr> {
    if !ctx
        .data()
        .database
        .set_admin(user.id, None, false)
        .await
        .map_err(|_| CommandRestrictErr::DoesntExist)?
    {
        return Err(CommandRestrictErr::DoesntExist);
    }

    Ok(())
}

#[must_use]
pub fn commands() -> [crate::Command; 6] {
    [
        allow_admin_cmd(),
        deny_admin_cmd(),
        allow_admin(),
        deny_admin(),
        bot_ban(),
        bot_unban(),
    ]
}
