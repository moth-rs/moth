use std::{collections::HashMap, time::Duration};

use serenity::all::{
    ChannelType, CreateMessage, CreatePoll, CreatePollAnswer, GuildChannel, MessageId, ThreadId,
};

use crate::{owner::admin, Context, Error};

#[lumi::command(
    prefix_command,
    check = "admin",
    category = "Admin - Commands",
    hide_in_help,
    guild_only
)]
pub async fn polls(ctx: Context<'_>, channel: GuildChannel) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    if channel.base.kind != ChannelType::Forum {
        ctx.say("Not a forum channel bestie.").await?;
        return Ok(());
    }

    let Some(tag) = channel
        .available_tags
        .iter()
        .find(|t| t.name == "Voting")
        .map(|t| t.id)
    else {
        ctx.say("Cannot find `Voting` tag.").await?;
        return Ok(());
    };

    let threads = guild_id.get_active_threads(ctx.http()).await?;

    if threads.has_more {
        ctx.say("WARNING: apparently there is more active threads?")
            .await?;
    }

    let archived_threads = channel
        .id
        .get_archived_public_threads(ctx.http(), None, None)
        .await?;

    if archived_threads.has_more {
        ctx.say(
            "WARNING: how the FUCK do you have more than the limit of active threads fetchable in \
             one request? I am not handling this do it yourself <:moth:1369814651193397338>",
        )
        .await?;
    }

    let mut file = read_map_from_json_file("polls.json").unwrap_or_default();

    for thread in threads.threads {
        if file.contains_key(&thread.id) {
            continue;
        }

        if thread.thread_metadata.locked() {
            continue;
        }

        if !thread.applied_tags.contains(&tag) {
            continue;
        }

        let Ok(starter_message) = thread
            .id
            .widen()
            .message(ctx.http(), thread.id.get().into())
            .await
        else {
            let _ = ctx
                .say(format!(
                    "Cannot fetch starter message of <#{}>, ignoring.",
                    thread.id
                ))
                .await;
            continue;
        };

        let name = thread.base.name.split('\'').next().unwrap_or("Unknown");
        let mut mod_position = None;
        for line in starter_message.content.lines() {
            if line.starts_with("**Position**") {
                if let Some(pos) = line.split("—").nth(1) {
                    let position = pos.trim();
                    mod_position = Some(position);
                }
            }
        }

        let poll = CreatePoll::new()
            .question(format!(
                "Accept {name} as {}",
                mod_position.unwrap_or("Unknown")
            ))
            .answers(vec![
                CreatePollAnswer::new().text("Yes"),
                CreatePollAnswer::new().text("No"),
                CreatePollAnswer::new().text("Neutral (don't count my vote)"),
            ])
            .duration(Duration::from_secs(86400 * 32));

        let msg = thread
            .send_message(ctx.http(), CreateMessage::new().poll(poll))
            .await?;

        file.insert(thread.id, msg.id);
    }

    write_map_to_json_file("polls.json", &file);

    ctx.say("Done!").await?;

    Ok(())
}

fn write_map_to_json_file(filename: &str, data: &HashMap<ThreadId, MessageId>) {
    let json = serde_json::to_string(data).expect("Failed to serialize");
    std::fs::write(filename, json).expect("Failed to write to file");
}

fn read_map_from_json_file(filename: &str) -> Option<HashMap<ThreadId, MessageId>> {
    let content = std::fs::read_to_string(filename).ok()?;
    serde_json::from_str(&content).ok()
}
#[must_use]
pub fn commands() -> [crate::Command; 1] {
    [polls()]
}
