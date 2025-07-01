use crate::{Command, Data};

use std::sync::Arc;

use lumi::serenity_prelude::User;

pub enum CommandRestrictErr {
    CommandNotFound,
    AlreadyExists,
    DoesntExist,
    FrameworkOwner,
    NotOwnerCommand,
}

pub async fn handle_allow_cmd(
    commands: &[Command],
    data: &Arc<Data>,
    cmd_name: String,
    user: &User,
) -> Result<String, CommandRestrictErr> {
    // Check if the command or its aliases match a real command.
    let command_name = get_cmd_name(commands, &cmd_name)?;

    data.database
        .set_admin(user.id, Some(&command_name), true)
        .await
        .map_err(|_| CommandRestrictErr::DoesntExist)?;

    Ok(command_name)
}

pub fn get_cmd_name(
    commands: &[crate::Command],
    cmd_name: &str,
) -> Result<String, CommandRestrictErr> {
    let mut command_name = String::new();
    for command in commands {
        // If the command isn't an owner command, skip.
        if !command
            .category
            .as_deref()
            .is_some_and(|c| c.to_lowercase().starts_with("admin"))
        {
            continue;
        }

        if command.name == cmd_name.to_lowercase()
            || command
                .aliases
                .iter()
                .any(|alias| alias == &cmd_name.to_lowercase())
        {
            // Check if the command is an owner command.
            if !command
                .category
                .as_deref()
                .is_some_and(|c| c.to_lowercase().starts_with("owner"))
            {
                return Err(CommandRestrictErr::NotOwnerCommand);
            }

            // Commands that require you to be a framework owner are not supported.
            if command.owners_only {
                return Err(CommandRestrictErr::FrameworkOwner);
            }

            command_name.clone_from(&command.name.as_ref().to_owned());
            break;
        }
    }

    if command_name.is_empty() {
        return Err(CommandRestrictErr::CommandNotFound);
    }

    Ok(command_name)
}

pub async fn handle_deny_cmd(
    commands: &[crate::Command],
    data: &Arc<Data>,
    cmd_name: &str,
    user: &User,
) -> Result<String, CommandRestrictErr> {
    // Check if the command or its aliases match a real command.
    let command_name = get_cmd_name(commands, cmd_name)?;

    // TODO: handle errors better.
    data.database
        .set_admin(user.id, Some(&command_name), false)
        .await
        .map_err(|_| CommandRestrictErr::DoesntExist)?;

    Ok(command_name)
}
