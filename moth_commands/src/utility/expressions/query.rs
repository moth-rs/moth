use super::{Expression, ExpressionCounts};
use crate::Error;
use moth_core::data::database::{Database, EmoteUsageType};
use serenity::all::GuildId;
use sqlx::query_as;

pub(super) async fn handle_expression_query(
    database: &Database,
    expression: &Expression<'_>,
    guild_id: GuildId,
    types: &[EmoteUsageType],
) -> Result<Vec<ExpressionCounts>, Error> {
    let guild_id = database.get_guild(guild_id).await?;

    let results = match expression {
        Expression::Id(id) | Expression::Emote((id, _)) => {
            let id = *id;
            query_as!(
                ExpressionCounts,
                "SELECT eu.user_id, COUNT(eu.id) AS reaction_count FROM emote_usage eu JOIN \
                 emotes e ON eu.emote_id = e.id WHERE eu.usage_type = ANY($3) AND e.discord_id = \
                 $1 AND eu.guild_id = $2 GROUP BY  eu.user_id ORDER BY  reaction_count DESC",
                id as i64,
                guild_id,
                types as &[EmoteUsageType]
            )
            .fetch_all(&database.db)
            .await?
        }
        Expression::Name(string) => {
            query_as!(
                ExpressionCounts,
                "SELECT eu.user_id, COUNT(eu.id) AS reaction_count FROM emote_usage eu JOIN \
                 emotes e ON eu.emote_id = e.id WHERE eu.usage_type = ANY($3) AND e.emote_name = \
                 $1 AND eu.guild_id = $2 GROUP BY eu.user_id ORDER BY reaction_count DESC",
                string,
                guild_id,
                types as &[EmoteUsageType]
            )
            .fetch_all(&database.db)
            .await?
        }
        Expression::Standard(string) => {
            query_as!(
                ExpressionCounts,
                "SELECT eu.user_id, COUNT(eu.id) AS reaction_count FROM emote_usage eu JOIN \
                 emotes e ON eu.emote_id = e.id WHERE eu.usage_type = ANY($3) AND e.emote_name = \
                 $1 AND eu.guild_id = $2 AND e.discord_id IS NULL GROUP BY eu.user_id ORDER BY \
                 reaction_count DESC",
                string,
                guild_id,
                types as &[EmoteUsageType]
            )
            .fetch_all(&database.db)
            .await?
        }
    };

    Ok(results)
}
