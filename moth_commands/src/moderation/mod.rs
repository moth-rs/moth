use std::collections::HashSet;

use crate::{Error, PrefixContext};

use poise::serenity_prelude as serenity;
use small_fixed_array::FixedString;

/// Purge messages in a channel.
#[poise::command(
    prefix_command,
    category = "Moderation - Purge",
    required_permissions = "MANAGE_MESSAGES",
    required_bot_permissions = "MANAGE_MESSAGES | VIEW_CHANNEL | READ_MESSAGE_HISTORY",
    hide_in_help
)]
pub async fn purge(
    ctx: PrefixContext<'_>,
    limit: u8,
    #[rest] command: Option<String>,
) -> Result<(), Error> {
    // would like a macro, as seen below.
    if ctx.author().id.get() != 158567567487795200 {
        msg_or_reaction(
            ctx,
            "This command is restricted to Moxy only as this is some actual witchcraft",
            "❌",
        )
        .await;
        return Ok(());
    }

    if !(2..=100).contains(&limit) {
        reaction_or_msg(ctx, "Can't purge 1 or more than 100 messages.", "❓").await;
        return Ok(());
    }

    // TODO: before/after stuff
    let messages = ctx
        .channel_id()
        .messages(
            ctx,
            serenity::GetMessages::new().before(ctx.msg.id).limit(limit),
        )
        .await?;

    let mut deleted = HashSet::new();

    let Some(command) = command else {
        ctx.channel_id()
            .delete_messages(
                ctx.http(),
                &messages.iter().map(|m| m.id).collect::<Vec<_>>(),
                Some(&format!(
                    "Purged by {} (ID:{})",
                    ctx.author().name,
                    ctx.author().id
                )),
            )
            .await?;

        return Ok(());
    };

    let groups = parse_command(&command);

    for group in dbg!(groups) {
        match group.modifier {
            Modifier::User => {
                let Some(user) = serenity::parse_user_mention(group.content) else {
                    reaction_or_msg(ctx, "Cannot parse users.", "❓").await;
                    return Ok(());
                };

                for msg in &messages {
                    if msg.author.id == user {
                        deleted.insert(msg.id);
                    }
                }
            }
            Modifier::Match => {
                for msg in &messages {
                    if msg.content.contains(group.content) {
                        deleted.insert(msg.id);
                    }
                }
            }
            Modifier::StartsWith => {
                for msg in &messages {
                    if msg.content.starts_with(group.content) {
                        deleted.insert(msg.id);
                    }
                }
            }
            Modifier::EndsWith => {
                for msg in &messages {
                    if msg.content.ends_with(group.content) {
                        deleted.insert(msg.id);
                    }
                }
            }
            Modifier::Links => {}
            Modifier::Invites => {}
            Modifier::Images => {}
        }
    }

    ctx.channel_id()
        .delete_messages(
            ctx.http(),
            &deleted.iter().copied().collect::<Vec<_>>(),
            Some(&format!(
                "Purged by {} (ID:{})",
                ctx.author().name,
                ctx.author().id
            )),
        )
        .await?;

    Ok(())
}

#[derive(Debug)]
enum Modifier {
    User,
    Match,
    StartsWith,
    EndsWith,
    Links,
    Invites,
    Images,
}

#[derive(Debug)]
struct ModifierGroup<'a> {
    modifier: Modifier,
    content: &'a str,
    negated: bool,
}

macro_rules! set_modifier_match {
    ($word:expr, $result:expr, $current_modifier:expr, $current_start:expr, $command:expr, $index:expr, $is_negated:expr, $( $pattern:expr => $modifier:expr ),* $(,)?) => {
        match $word {
            $(
                $pattern => {
                    // If there's a current modifier, finalize the group with the previous content
                    if let Some(modifier) = $current_modifier.take() {
                        let content = &$command[*$current_start..$index].trim();
                        $result.push(ModifierGroup {
                            modifier,
                            content,
                            negated: $is_negated,
                        });
                    }

                    // Start a new modifier group
                    *$current_modifier = Some($modifier);
                    *$current_start = $index + $word.len() + 1;  // Move start position after this word
                },
            )*
            _ => {},
        }
    };
}

fn parse_command(command: &str) -> Vec<ModifierGroup<'_>> {
    let mut result = Vec::new();
    let mut current_modifier: Option<Modifier> = None;
    let mut current_start = 0;
    let mut is_negated = false;

    let mut in_special_group = false;

    for (index, word) in command.split_whitespace().enumerate() {
        let (is_negated_next, stripped_word) = if let Some(stripped) = word.strip_prefix('!') {
            (true, stripped)
        } else {
            (false, word)
        };

        if !in_special_group {
            set_modifier_match!(
                stripped_word,
                &mut result,
                &mut current_modifier,
                &mut current_start,
                command,
                index,
                is_negated,
                "images" => Modifier::Images,
                "links" => Modifier::Links,
                "user" => Modifier::User,
                "invites" => Modifier::Invites,
            );
        }

        // Handle negation state
        is_negated = is_negated_next;
    }

    // Finalize the last modifier, if any
    if let Some(modifier) = current_modifier {
        let content = &command[current_start..].trim();
        result.push(ModifierGroup {
            modifier,
            content,
            negated: is_negated,
        });
    }

    result
}

async fn reaction_or_msg(ctx: PrefixContext<'_>, msg: &str, reaction: &str) {
    message_react(ctx, true, msg, reaction).await;
}

async fn msg_or_reaction(ctx: PrefixContext<'_>, msg: &str, reaction: &str) {
    message_react(ctx, false, msg, reaction).await;
}

async fn message_react(ctx: PrefixContext<'_>, flipped: bool, msg: &str, reaction: &str) {
    let (message, react) = has_permissions(&ctx);

    if (flipped && react) || (!flipped && !message) {
        // Prioritize reaction
        if react {
            let _ = ctx
                .msg
                .react(
                    ctx.http(),
                    serenity::ReactionType::Unicode(FixedString::from_str_trunc(reaction)),
                )
                .await;
        }
    } else {
        // Fallback to sending a message
        if message {
            let _ = ctx.say(msg).await;
        }
    }
}

fn has_permissions(ctx: &PrefixContext) -> (bool, bool) {
    if let Some(guild) = ctx.guild() {
        let mut from_thread = false;

        let channel = guild.channels.get(&ctx.channel_id()).or_else(|| {
            guild
                .threads
                .iter()
                .find(|t| t.id == ctx.channel_id())
                .inspect(|_| {
                    from_thread = true;
                })
        });

        if let Some(channel) = channel {
            let permissions = guild.user_permissions_in(
                channel,
                guild.members.get(&ctx.cache().current_user().id).unwrap(),
            );

            if from_thread {
                return (
                    permissions.send_messages_in_threads(),
                    permissions.add_reactions(),
                );
            }

            return (permissions.send_messages(), permissions.add_reactions());
        }
    }

    (false, false)
}

#[must_use]
pub fn commands() -> [crate::Command; 1] {
    [purge()]
}
