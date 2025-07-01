pub mod cache;
pub mod checks;
pub mod cooldowns;
pub mod database;
pub mod other;
pub mod phil;
pub mod poll;
pub mod presence;
pub mod roles;

use crate::{Context, Error};

/// ALL owner commands should have a category that starts with owner.
/// Well, not all, only ones that are intended to be given out to trusted users.
#[must_use]
pub fn commands() -> Vec<crate::Command> {
    {
        cache::commands()
            .into_iter()
            .chain(checks::commands())
            .chain(database::commands())
            .chain(presence::commands())
            .chain(other::commands())
            .chain(cooldowns::commands())
            .chain(phil::commands())
            .chain(poll::commands())
            .chain(roles::commands())
            .collect()
    }
}

/// I use this check instead of the default `owners_only` check
/// When i want to be able to temporarily give access to specific owner commands
/// This executes after `command_check` is executed, so this works.
pub async fn admin(ctx: Context<'_>) -> Result<bool, Error> {
    let user_id = &ctx.author().id;
    // Owners will always be able to execute.
    if ctx.framework().options.owners.contains(user_id) {
        return Ok(true);
    }

    ctx.data()
        .database
        .check_admin(ctx.author().id, &ctx.command().name)
}
