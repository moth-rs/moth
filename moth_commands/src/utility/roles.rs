use aformat::aformat;
use lumi::StrArg;
use serenity::all::{Colour, EditRole, Mention, Permissions, RoleColours, RoleId};
use small_fixed_array::FixedString;

use crate::{ApplicationContext, Error};

const TRANSCENDENT: RoleId = RoleId::new(1063901598570520626);

// im too lazy to make slash argument trait implementation right now.
fn parse_hex_colour(s: &str) -> Result<Colour, Error> {
    let hex = s.trim_start_matches('#');

    if hex.len() != 6 {
        return Err("Hex color must be exactly 6 characters long".into());
    }

    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Hex color contains invalid characters".into());
    }

    let value = u32::from_str_radix(hex, 16).map_err(|_| "Invalid hex color")?;

    let r = ((value >> 16) & 0xFF) as f32 / 255.0;
    let g = ((value >> 8) & 0xFF) as f32 / 255.0;
    let b = (value & 0xFF) as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let lightness = f32::midpoint(max, min);

    if lightness < 0.05 {
        return Err("This colour is too dark, please pick something lighter!".into());
    }

    Ok(Colour::new(value))
}

fn option_result_to_option<T, E>(opt: Option<Result<T, E>>) -> Result<Option<T>, E> {
    opt.map_or(Ok(None), |res| res.map(Some))
}

/// Set your custom gradient! A perk for being level 100.
#[lumi::command(
    slash_command,
    rename = "set-gradient",
    category = "Roles",
    install_context = "Guild",
    interaction_context = "Guild",
    required_bot_permissions = "MANAGE_ROLES",
    // TODO: manual cooldowns this and exclude parse errors.
    member_cooldown = "15"
)]
pub async fn set_gradient(
    ctx: ApplicationContext<'_>,
    #[description = "A hex code."] primary_colour: Option<StrArg<FixedString<u8>>>,
    #[description = "A hex code."] secondary_colour: Option<StrArg<FixedString<u8>>>,
) -> Result<(), Error> {
    if primary_colour.is_some() ^ secondary_colour.is_some() {
        ctx.say("You must select either no colours (to reset) or both colours.")
            .await?;

        return Ok(());
    }

    let primary = option_result_to_option(primary_colour.map(|arg| parse_hex_colour(&arg.0)))?
        .unwrap_or_default();
    let secondary = option_result_to_option(secondary_colour.map(|arg| parse_hex_colour(&arg.0)))?;

    let Some(member) = &ctx.interaction.member else {
        ctx.say("Discord API is having a moment?").await?;
        return Ok(());
    };

    ctx.defer_ephemeral().await?;

    if !member.roles.contains(&TRANSCENDENT) {
        ctx.say(&*aformat!(
            "You must have the {} role to run this command.",
            Mention::Role(TRANSCENDENT)
        ))
        .await?;

        return Ok(());
    }

    let existing_role_id = sqlx::query!(
        "SELECT role_id FROM transcendent_roles WHERE user_id = $1",
        &ctx.data().database.get_user(ctx.author().id).await?.id
    )
    .fetch_optional(&ctx.data().database.db)
    .await?
    .map(|r| RoleId::new(r.role_id as u64));

    if let Some(role_id) = existing_role_id {
        let role_still_exists = {
            let Some(guild) = ctx.guild() else {
                ctx.say("Cannot resolve guild from cache, please report this to jamesbt365.")
                    .await?;
                return Ok(());
            };

            guild.roles.contains_key(&role_id)
        };

        if role_still_exists {
            // we should add to the user if they don't have it, then return because they probably don't want to set these colours.

            if !member.roles.contains(&role_id) {
                ctx.http()
                    .add_member_role(
                        ctx.guild_id().unwrap(),
                        ctx.author().id,
                        role_id,
                        Some("User had their gradient perk readded."),
                    )
                    .await?;

                ctx.say(
                    "Assigned your gradient role to you as you did not have it, send this command \
                     again if you want to change the colours.",
                )
                .await?;

                return Ok(());
            }

            let _ = ctx
                .guild_id()
                .unwrap()
                .edit_role(
                    ctx.http(),
                    role_id,
                    EditRole::new().colours(RoleColours {
                        primary_colour: primary,
                        secondary_colour: secondary,
                        tertiary_colour: None,
                    }),
                )
                .await?;

            let is_unset = matches!(primary.0, 0x000000)
                && matches!(secondary.map(|v| v.0), Some(0x000000) | None);

            if is_unset {
                ctx.say("Unset your gradient colour.").await?;
            } else {
                ctx.say(&*aformat!(
                    "Your colour on <@&{role_id}> has been changed to your choice."
                ))
                .await?;
            }
        } else {
            // remove deleted role from database
            let _ = sqlx::query!(
                "DELETE FROM transcendent_roles WHERE role_id = $1",
                role_id.get() as i64
            )
            .execute(&ctx.data().database.db)
            .await;

            create_role_and_insert(ctx, primary, secondary).await?;
        }
    } else {
        create_role_and_insert(ctx, primary, secondary).await?;
    }

    Ok(())
}

/// creates a role, inserts it into the database and then assigns it to the user
async fn create_role_and_insert(
    ctx: ApplicationContext<'_>,
    colour_primary: Colour,
    colour_secondary: Option<Colour>,
) -> Result<(), &'static str> {
    let Some(poop_position) = ({
        let Some(guild) = ctx.guild() else {
            return Err("Could not resolve guild in cache, please contact jamesbt365 about this.");
        };

        // idk how else to like do this.
        guild
            .roles
            .get(&RoleId::new(768165851680473098))
            .map(|r| r.position)
    }) else {
        return Err("Cannot find role to based position off.");
    };

    let reserved_id: i64 = sqlx::query_scalar("SELECT nextval('public.transcendent_roles_id_seq')")
        .fetch_one(&ctx.data().database.db)
        .await
        .map_err(|_| {
            "Could not reserve database information, please contact jamesbt365 about this."
        })?;

    let role_id = ctx
        .guild_id()
        .unwrap()
        .create_role(
            ctx.http(),
            EditRole::new()
                .name("TRANSCENDENT-GRADIENT")
                .position(poop_position)
                .colours(RoleColours {
                    primary_colour: colour_primary,
                    secondary_colour: colour_secondary,
                    tertiary_colour: None,
                })
                .permissions(Permissions::empty()),
        )
        .await
        .map_err(|_| "Error creating role, please report this to jamesbt365.")?
        .id;

    sqlx::query!(
        "INSERT INTO transcendent_roles (id, user_id, role_id) VALUES ($1, $2, $3)",
        reserved_id as i16,
        &ctx.data()
            .database
            .get_user(ctx.author().id)
            .await
            .map_err(|_| "Could not fetch user from database, you do NOT exist.")?
            .id,
        role_id.get() as i64,
    )
    .execute(&ctx.data().database.db)
    .await
    .map_err(|_| "Could not secure your role, please report this to jamesbt365.")?;

    ctx.http()
        .add_member_role(
            ctx.guild_id().unwrap(),
            ctx.author().id,
            role_id,
            Some("User has created their gradient role."),
        )
        .await
        .map_err(|_| "Could not assign role to you, I probably lack permissions!")?;

    let _ = ctx
        .say(&*aformat!(
            "Your role <@&{role_id}> has been created and assigned to you."
        ))
        .await;

    Ok(())
}

#[must_use]
pub fn commands() -> [crate::Command; 1] {
    [set_gradient()]
}
