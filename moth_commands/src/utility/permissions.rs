use crate::{Context, Error};
use lumi::serenity_prelude::{self as serenity, GuildChannel};
use std::fmt::Write;

enum Mentionable {
    Role(serenity::Role),
    User(Box<(serenity::User, Option<serenity::PartialMember>)>),
}

#[serenity::async_trait]
impl lumi::SlashArgument for Mentionable {
    async fn extract(
        _: &serenity::Context,
        _: &serenity::CommandInteraction,
        value: &serenity::ResolvedValue<'_>,
    ) -> Result<Mentionable, lumi::SlashArgError> {
        match *value {
            serenity::ResolvedValue::Role(val) => Ok(Mentionable::Role(val.clone())),
            serenity::ResolvedValue::User(user, member) => {
                Ok(Mentionable::User(Box::new((user.clone(), member.cloned()))))
            }
            _ => Err(lumi::SlashArgError::new_command_structure_mismatch(
                "Expected Role or User resolved value to form Mentionable.",
            )),
        }
    }

    fn create(builder: serenity::CreateCommandOption<'_>) -> serenity::CreateCommandOption<'_> {
        builder.kind(serenity::CommandOptionType::Mentionable)
    }
}

#[lumi::command(
    slash_command,
    install_context = "Guild|User",
    interaction_context = "Guild",
    category = "Utility",
    guild_only
)]
pub async fn permissions(
    ctx: Context<'_>,
    channel: Option<GuildChannel>,
    role_or_user: Mentionable,
) -> Result<(), Error> {
    let Mentionable::User(user) = role_or_user else {
        ctx.say(
            "Despite it looking like it given the arguments, peeking at a role directly is out of \
             scope right now.",
        )
        .await?;
        return Ok(());
    };

    let Some(channel) = channel else {
        ctx.say("I am too lazy to write code for this right now! (you will specify a channel)").await?;
        return Ok(())
    };


    let (user, Some(member)) = *user else {
        return Ok(());
    };

    // guild
    // channel (not including threads)

    enum CanManageRoles {
        No,
        YesImplicit,
        YesExplicit,
    }

    let permissions = {
        let Some(guild) = ctx.guild() else {
            ctx.say("Guild is not cached, cannot perform this action at this time.").await?;
            return Ok(())
        };

        guild.partial_member_permissions(user.id, &member);

        // i need to now check if MANAGE_ROLES is explicit or implilict, which means i need to iterate over the overwrites!
        // how the hell do i check if its explilict by a role/user? i just need to know what the hell is going on

        let mut can_role = None;
        let mut can_user = None;
        for overwrite in channel.permission_overwrites.iter() {
            match overwrite.kind {
                serenity::PermissionOverwriteType::Member(user_id) => {
                    if user_id == user.id {
                        if overwrite.allow.manage_roles() {
                            can_user = Some(true);
                            continue;
                        }

                        if overwrite.deny.manage_roles() {
                            can_user = Some(false);
                        }
                    }
                },
                serenity::PermissionOverwriteType::Role(role_id) => {
                    if member.roles.contains(&role_id) {
                        if overwrite.allow.manage_roles() {
                            can_role = Some(true);
                        }

                        // its already allowed on a role, it cannot be shut off by a role.
                        if can_role == Some(true) {
                            continue;
                        }

                        if overwrite.deny.manage_roles() {
                            can_role == Some(false)
                        }

                    }
                },
                _ => {},
            }
        }

        // now i need to combine the 2 Option<bools> to form an implicit enable/disable/implict state generator
        // idk how i'm gonna do it


    }

    Ok(())
}

// i want a second version of the above command to find out *what* has a specific permission

#[must_use]
pub fn commands() -> [crate::Command; 1] {
    [permissions()]
}
