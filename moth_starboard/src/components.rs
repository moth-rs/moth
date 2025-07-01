use std::{str::FromStr, sync::Arc};

use crate::{Data, Error};
use ::serenity::all::CreateInteractionResponseMessage;
use lumi::serenity_prelude as serenity;
use moth_core::data::database::StarboardStatus;

use super::starboard::starboard_message;

pub async fn handle_component(
    ctx: &serenity::Context,
    data: Arc<Data>,
    interaction: &serenity::ComponentInteraction,
) -> Result<(), Error> {
    if !data.starboard_config.active {
        return Ok(());
    }

    if !matches!(
        interaction.data.custom_id.as_str(),
        "starboard_accept" | "starboard_deny"
    ) {
        return Ok(());
    }

    if interaction.channel_id != data.starboard_config.queue_channel {
        return Ok(());
    }

    // in guild
    if !interaction
        .member
        .as_ref()
        .unwrap()
        .roles
        .contains(&data.starboard_config.allowed_role)
    {
        interaction
            .create_response(
                &ctx.http,
                serenity::CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content("You are not allowed to do this.")
                        .ephemeral(true),
                ),
            )
            .await?;

        return Ok(());
    }

    // on the race condition case i should probably send a response?
    if interaction.data.custom_id == "starboard_accept" {
        // create new message
        // run approve function
        if !data.database.handle_starboard(interaction.message.id) {
            let _ = accept(ctx, &data, interaction).await;
            data.database.stop_handle_starboard(&interaction.message.id);
        }
    } else if interaction.data.custom_id == "starboard_deny" {
        if !data.database.handle_starboard(interaction.message.id) {
            let _ = deny(ctx, &data, interaction).await;
            data.database.stop_handle_starboard(&interaction.message.id);
        }
    } else {
        return Ok(());
    }

    Ok(())
}

async fn accept(
    ctx: &serenity::Context,
    data: &Arc<Data>,
    interaction: &serenity::ComponentInteraction,
) -> Result<(), Error> {
    let mut starboard = data
        .database
        .get_starboard_msg_by_starboard_id(interaction.message.id)
        .await?;

    starboard.starboard_status = StarboardStatus::Accepted;

    let builder = CreateInteractionResponseMessage::new()
        .components(&[])
        .content(format!("Approved by <@{}>", interaction.user.id));

    interaction
        .create_response(
            &ctx.http,
            serenity::CreateInteractionResponse::UpdateMessage(builder),
        )
        .await?;

    let new_msg = data
        .starboard_config
        .post_channel
        .send_message(&ctx.http, starboard_message(ctx, data, &starboard))
        .await?;

    let _ = new_msg
        .react(
            &ctx.http,
            serenity::ReactionType::Unicode(
                small_fixed_array::FixedString::from_str(&data.starboard_config.star_emoji)
                    .unwrap(),
            ),
        )
        .await;

    // required to copy the bot id, for whatever reason rust wont copy it here without explicitly saying so.
    #[expect(unused_braces)]
    // TODO: rewrite.
    data.database
        .approve_starboard(
            { ctx.cache.current_user().id },
            interaction.message.id,
            interaction.message.channel_id,
            new_msg.id,
            new_msg.channel_id,
        )
        .await?;

    Ok(())
}

async fn deny(
    ctx: &serenity::Context,
    data: &Arc<Data>,
    interaction: &serenity::ComponentInteraction,
) -> Result<(), Error> {
    let builder = CreateInteractionResponseMessage::new()
        .components(&[])
        .content(format!("Denied by <@{}>", interaction.user.id));

    interaction
        .create_response(
            &ctx.http,
            serenity::CreateInteractionResponse::UpdateMessage(builder),
        )
        .await?;

    data.database.deny_starboard(interaction.message.id).await?;

    Ok(())
}
