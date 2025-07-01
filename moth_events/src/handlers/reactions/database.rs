use ::serenity::all::{Context, Reaction, ReactionType, UserId};
use chrono::Utc;
use sqlx::query;

use crate::Error;

use moth_core::data::{database::EmoteUsageType, structs::Data};

async fn insert_emote_usage(
    ctx: &Context,
    user_id: UserId,
    reaction: &Reaction,
    usage_type: EmoteUsageType,
) -> Result<(), Error> {
    let database = &ctx.data_ref::<Data>().database;

    let (name, id) = match &reaction.emoji {
        ReactionType::Custom {
            animated: _,
            id,
            name,
        } => {
            let Some(name) = name else { return Ok(()) };

            (name, Some(id.get() as i64))
        }
        ReactionType::Unicode(string) => (string, None),
        _ => return Ok(()),
    };

    // reaction user
    let user_id = database.get_user(user_id).await?.id;

    // to get the reaction's message's id
    let reaction_message_id =
        if let Some(msg) = database.get_cached_message(&reaction.message_id) {
            msg
        } else {
            let message = reaction.message(&ctx).await?;
            database
                .get_message(
                    message.id,
                    message.channel_id,
                    message.guild_id,
                    message.author.id,
                )
                .await?
        }
        .id;

    let id = if let Some(id) = id {
        let id = query!(
            "INSERT INTO emotes (emote_name, discord_id) VALUES ($1, $2) ON CONFLICT (discord_id) \
             DO UPDATE SET emote_name = EXCLUDED.emote_name RETURNING id",
            &name.as_str(),
            id
        )
        .fetch_one(&database.db)
        .await?;
        id.id
    } else {
        let id = query!(
            r#"
            WITH input_rows(emote_name) AS (
                VALUES ($1::text)
            ),
            ins AS (
                INSERT INTO emotes (emote_name)
                SELECT emote_name FROM input_rows
                ON CONFLICT (emote_name) WHERE discord_id IS NULL DO NOTHING
                RETURNING id
            )
            SELECT id AS "id!" FROM ins
            UNION ALL
            SELECT e.id AS "id!"
            FROM emotes e
            JOIN input_rows i ON e.emote_name = i.emote_name
            WHERE e.discord_id IS NULL;
            "#,
            &name.as_str(),
        )
        .fetch_one(&database.db)
        .await?;

        id.id
    };

    query!(
        "INSERT INTO emote_usage (emote_id, message_id, user_id, used_at, usage_type) VALUES ($1, \
         $2, $3, $4, $5)",
        id,
        reaction_message_id,
        user_id,
        Utc::now(),
        usage_type as _,
    )
    .execute(&database.db)
    .await?;

    Ok(())
}

pub(super) async fn insert_addition(
    ctx: &Context,
    user_id: UserId,
    reaction: &Reaction,
) -> Result<(), Error> {
    insert_emote_usage(ctx, user_id, reaction, EmoteUsageType::Reaction).await?;
    Ok(())
}
