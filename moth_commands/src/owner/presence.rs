use lumi::serenity_prelude::{ActivityData, ActivityType};

use crate::{owner::admin, Context, Error};

use small_fixed_array::FixedString;

#[derive(Debug, lumi::ChoiceParameter)]
pub enum OnlineStatus {
    Online,
    Idle,
    #[name = "Do Not Disturb"]
    #[name = "dnd"]
    DoNotDisturb,
    Invisible,
}

#[lumi::command(
    prefix_command,
    check = "admin",
    category = "Admin - Presence",
    track_edits,
    hide_in_help
)]
pub async fn status(
    ctx: Context<'_>,
    #[description = "What online status should I have?"] status_type: OnlineStatus,
) -> Result<(), Error> {
    let s = ctx.serenity_context();
    match status_type {
        OnlineStatus::Online => {
            s.online();
        }
        OnlineStatus::Idle => {
            s.idle();
        }
        OnlineStatus::DoNotDisturb => {
            s.dnd();
        }
        OnlineStatus::Invisible => {
            s.invisible();
        }
    }

    ctx.say(format!("Updating status to: **{status_type:?}**."))
        .await?;

    Ok(())
}

#[lumi::command(
    rename = "reset-presence",
    prefix_command,
    category = "Admin - Presence",
    check = "admin",
    hide_in_help
)]
pub async fn reset_presence(ctx: Context<'_>) -> Result<(), Error> {
    ctx.serenity_context().reset_presence();
    ctx.say("Resetting the current presence...").await?;

    Ok(())
}

#[lumi::command(
    rename = "set-activity",
    prefix_command,
    category = "Admin - Presence",
    check = "admin",
    track_edits,
    hide_in_help
)]
pub async fn set_activity(
    ctx: Context<'_>,
    #[description = "The activity name"] name: String,
    #[description = "The activity type"] activity_type: String,
    #[description = "Custom status (optional)"] custom_status: Option<String>,
) -> Result<(), Error> {
    let activity_type_enum = match activity_type.to_lowercase().as_str() {
        "streaming" => ActivityType::Streaming,
        "listening" => ActivityType::Listening,
        "watching" => ActivityType::Watching,
        "custom" => ActivityType::Custom,
        "competing" => ActivityType::Competing,
        _ => ActivityType::Playing,
    };

    let status = custom_status.map(|s| FixedString::from_str_trunc(&s));

    let activity_data = ActivityData {
        name: FixedString::from_str_trunc(&name),
        kind: activity_type_enum,
        state: status,
        url: None,
    };

    ctx.serenity_context().set_activity(Some(activity_data));

    Ok(())
}

#[must_use]
pub fn commands() -> [crate::Command; 3] {
    [status(), reset_presence(), set_activity()]
}
