use regex::Regex;
use sqlx::query;
use std::sync::LazyLock;
use unicode_segmentation::UnicodeSegmentation;

use crate::Error;
use lumi::serenity_prelude::Message;
use moth_core::data::database::{Database, EmoteUsageType};

pub static EMOJI_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<(a)?:([a-zA-Z0-9_]{2,32}):(\d{1,20})>").unwrap());

fn get_emojis_in_msg(msg: &str) -> impl Iterator<Item = &str> {
    msg.graphemes(true)
        .filter(|g| emojis::get(g).is_some())
        .take(3)
}

pub(super) async fn insert_message(database: &Database, message: &Message) -> Result<(), Error> {
    let mut transaction = database.db.begin().await?;

    let mut unicode_emojis = get_emojis_in_msg(&message.content).peekable();
    let has_unicode_emoji = unicode_emojis.peek().is_some();

    let mut custom_captures = EMOJI_REGEX.captures_iter(&message.content).peekable();
    let has_custom_emote = custom_captures.peek().is_some();

    if !has_unicode_emoji && !has_custom_emote && message.sticker_items.is_empty() {
        return Ok(());
    }

    let message_data = database
        .get_message(
            message.id,
            message.channel_id,
            message.guild_id,
            message.author.id,
        )
        .await?;

    if message.guild_id.is_some() {
        for sticker in &message.sticker_items {
            let sticker_id = sticker.id.get() as i64;
            query!(
                "INSERT INTO stickers (sticker_id, sticker_name) VALUES ($1, $2) ON CONFLICT \
                 (sticker_id) DO NOTHING",
                sticker_id,
                &*sticker.name
            )
            .execute(&mut *transaction)
            .await?;

            query!(
                "INSERT INTO sticker_usage (message_id, sticker_id, user_id, guild_id, used_at) \
                 VALUES ($1, $2, $3, $4, $5)",
                message_data.id,
                sticker_id,
                message_data.user_id,
                message_data.guild_id,
                *message.id.created_at()
            )
            .execute(&mut *transaction)
            .await?;
        }

        // this should be rewritten as to not attempt to insert the same emoji back to back into the emotes table.
        // i can support this later when i do caches (TODO)
        for captures in custom_captures.take(3) {
            let Ok(id) = &captures[3].parse::<u64>() else {
                println!("Failed to parse id for custom emote: {}", &captures[3]);
                continue;
            };
            // &captures[2] is name.
            // &captures[3] is id.
            let id = query!(
                r#"
                    WITH input_rows(emote_name, discord_id) AS (
                        VALUES ($1::text, $2::bigint)
                    ),
                    ins AS (
                        INSERT INTO emotes (emote_name, discord_id)
                        SELECT emote_name, discord_id FROM input_rows
                        ON CONFLICT (emote_name, discord_id) DO NOTHING
                        RETURNING id
                    )
                    SELECT id AS "id!" FROM ins
                    UNION ALL
                    SELECT e.id AS "id!" FROM emotes e
                    JOIN input_rows i
                    ON e.emote_name = i.emote_name AND e.discord_id = i.discord_id;
                    "#,
                &captures[2],
                *id as i64
            )
            .fetch_one(&mut *transaction)
            .await?;

            query!(
                "INSERT INTO emote_usage (message_id, emote_id, user_id, guild_id,
                 used_at, usage_type) VALUES ($1, $2, $3, $4, $5, $6)",
                message_data.id,
                id.id,
                message_data.user_id,
                message_data.guild_id,
                *message.id.created_at(),
                EmoteUsageType::Message as _,
            )
            .execute(&mut *transaction)
            .await?;
        }

        for emoji in unicode_emojis {
            let id = query!(
                r#"
                WITH input_rows(emote_name) AS (
                    VALUES ($1::text)
                ),
                ins AS (
                    INSERT INTO emotes (emote_name, discord_id)
                    SELECT emote_name, NULL FROM input_rows
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
                emoji
            )
            .fetch_one(&mut *transaction)
            .await?;

            query!(
                "INSERT INTO emote_usage (message_id, emote_id, user_id, guild_id,
                 used_at, usage_type) VALUES ($1, $2, $3, $4, $5, $6)",
                message_data.id,
                id.id,
                message_data.user_id,
                message_data.guild_id,
                *message.id.created_at(),
                EmoteUsageType::Message as _,
            )
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
    }

    Ok(())
}
