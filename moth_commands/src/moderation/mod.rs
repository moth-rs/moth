use std::{collections::HashSet, time::Duration};

use crate::{Error, PrefixContext};

use moth_events::handlers::messages::invites::INVITE;
use poise::serenity_prelude as serenity;
use serenity::all::MessageId;
use small_fixed_array::FixedString;

/// Purge messages in a channel.
#[poise::command(
    rename = "purge-in",
    prefix_command,
    category = "Moderation - Purge",
    required_permissions = "MANAGE_MESSAGES",
    required_bot_permissions = "MANAGE_MESSAGES | VIEW_CHANNEL | READ_MESSAGE_HISTORY",
    hide_in_help
)]
pub async fn purge_in(
    ctx: PrefixContext<'_>,
    seconds: u16,
    limit: u8,
    command: Option<PurgeArgs>,
) -> Result<(), Error> {
    if seconds > 300 {
        reaction_or_msg(ctx, "Cannot wait more than 5 minutes to purge.", "❌").await;
    }

    match purge_prep(ctx, limit, command).await? {
        Some(messages) => {
            tokio::time::sleep(Duration::from_secs(seconds.into())).await;
            delete_messages(&ctx, messages).await?;
        }
        None => return Ok(()),
    }

    Ok(())
}

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
    command: Option<PurgeArgs>,
) -> Result<(), Error> {
    match purge_prep(ctx, limit, command).await? {
        Some(messages) => {
            delete_messages(&ctx, messages).await?;
        }
        None => return Ok(()),
    }

    Ok(())
}

static USER_REGEX: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(<@!?(\d+)>)|(\d{16,20})").unwrap());

async fn purge_prep(
    ctx: PrefixContext<'_>,
    limit: u8,
    command: Option<PurgeArgs>,
) -> Result<Option<HashSet<MessageId>>, Error> {
    if !(2..=100).contains(&limit) {
        reaction_or_msg(ctx, "Can't purge 1 or more than 100 messages.", "❓").await;
        return Ok(None);
    }

    let messages = ctx
        .channel_id()
        .messages(
            ctx,
            serenity::GetMessages::new().before(ctx.msg.id).limit(limit),
        )
        .await?;

    let mut deleted = HashSet::new();

    let Some(command) = command else {
        for message in messages {
            deleted.insert(message.id);
        }

        return Ok(Some(deleted));
    };

    for group in dbg!(command.0) {
        match group.modifier {
            Modifier::User => {
                let mut users = Vec::with_capacity(1);

                for caps in USER_REGEX.captures_iter(&group.content) {
                    if let Some(id) = caps.get(2).or_else(|| caps.get(3)) {
                        users.push(id.as_str().parse::<serenity::UserId>().unwrap());
                    }
                }

                if users.is_empty() {
                    reaction_or_msg(ctx, "Cannot parse users.", "❓").await;
                    return Ok(None);
                }

                for msg in &messages {
                    let matches = users.contains(&msg.author.id);

                    if matches != group.negated {
                        deleted.insert(msg.id);
                    }
                }
            }
            Modifier::Match => {
                for msg in &messages {
                    let matches = msg.content.contains(&group.content);
                    if matches != group.negated {
                        deleted.insert(msg.id);
                    }
                }
            }
            Modifier::StartsWith => {
                for msg in &messages {
                    let matches = msg.content.starts_with(&group.content);
                    if matches != group.negated {
                        deleted.insert(msg.id);
                    }
                }
            }
            Modifier::EndsWith => {
                for msg in &messages {
                    let matches = msg.content.ends_with(&group.content);
                    if matches != group.negated {
                        deleted.insert(msg.id);
                    }
                }
            }
            Modifier::Links => {
                for msg in &messages {
                    let matches =
                        msg.content.contains("http://") || msg.content.contains("https://");
                    if matches != group.negated {
                        deleted.insert(msg.id);
                    }
                }
            }
            Modifier::Invites => {
                for msg in &messages {
                    let matches = INVITE.is_match(&msg.content);
                    if matches != group.negated {
                        deleted.insert(msg.id);
                    }
                }
            }
            Modifier::Attachments => {
                for msg in &messages {
                    let matches = !msg.attachments.is_empty();
                    if matches != group.negated {
                        deleted.insert(msg.id);
                    }
                }
            }
            Modifier::Bot => {
                for msg in &messages {
                    let matches = msg.author.bot();

                    if matches != group.negated {
                        deleted.insert(msg.id);
                    }
                }
            }
        }
    }

    Ok(Some(deleted))
}

async fn delete_messages(
    ctx: &PrefixContext<'_>,
    mut deleted: HashSet<MessageId>,
) -> Result<(), Error> {
    let reason = &format!("Purged by {} (ID:{})", ctx.author().name, ctx.author().id);

    if deleted.len() > 99 {
        let _ = ctx.msg.delete(ctx.http(), Some(reason)).await;
    } else {
        deleted.insert(ctx.msg.id);
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

#[derive(Debug, Copy, Clone)]
enum Modifier {
    User,
    Bot,
    Match,
    StartsWith,
    EndsWith,
    Links,
    Invites,
    Attachments,
}

impl Modifier {
    // This method tries to match a modifier at the start of a string in a case-insensitive way
    fn match_prefix(s: &str) -> Option<(Modifier, usize)> {
        let modifiers = [
            (Modifier::User, "user"),
            (Modifier::Match, "match"),
            (Modifier::StartsWith, "startswith"),
            (Modifier::EndsWith, "endswith"),
            (Modifier::Links, "links"),
            (Modifier::Invites, "invites"),
            (Modifier::Attachments, "attachments"),
            (Modifier::Bot, "bots"),
        ];

        let s_lower = s.to_lowercase();

        for (modifier, name) in &modifiers {
            if s_lower.starts_with(name) {
                return Some((*modifier, name.len()));
            }
        }
        None
    }

    fn is_special(self) -> bool {
        matches!(
            self,
            Modifier::Match | Modifier::StartsWith | Modifier::EndsWith
        )
    }
}

#[derive(Debug)]
struct ModifierGroup {
    modifier: Modifier,
    content: String,
    negated: bool,
}

#[derive(Debug)]
struct PurgeArgs(pub Vec<ModifierGroup>);

#[serenity::async_trait]
impl<'a> poise::PopArgument<'a> for PurgeArgs {
    async fn pop_from(
        args: &'a str,
        attachment_index: usize,
        _ctx: &serenity::Context,
        _msg: &serenity::Message,
    ) -> Result<(&'a str, usize, Self), (Box<dyn std::error::Error + Send + Sync>, Option<String>)>
    {
        let mut rest = args.trim_start();

        if rest.is_empty() {
            return Err((poise::TooFewArguments::default().into(), None));
        }

        let mut groups = Vec::new();
        let mut current_modifier = None;
        let mut current_content: Vec<String> = Vec::new();

        let mut special_found = false;

        let mut negated = false;
        while !rest.is_empty() {
            if rest.starts_with('!') {
                rest = &rest[1..];
                negated = true;
            }

            // Try to match a modifier
            if let Some((modifier, modifier_len)) = Modifier::match_prefix(rest) {
                // new modifier, flush out old.
                flush(
                    &mut groups,
                    &mut current_modifier,
                    &mut current_content,
                    &mut negated,
                );

                rest = rest[modifier_len..].trim_start();
                current_modifier = Some(modifier);
                current_content.clear();

                // modifier is special, basically consume all content.
                if modifier.is_special() {
                    groups.push(ModifierGroup {
                        modifier,
                        content: rest.to_string(),
                        negated,
                    });

                    special_found = true;

                    break;
                }
            } else {
                let content_end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
                current_content.push(rest[..content_end].to_string());
                rest = rest[content_end..].trim_start();
            }
        }

        // Flush remaining content.
        if !special_found {
            flush(
                &mut groups,
                &mut current_modifier,
                &mut current_content,
                &mut negated,
            );
        }

        Ok(("", attachment_index, PurgeArgs(groups)))
    }
}

fn flush(
    groups: &mut Vec<ModifierGroup>,
    current_modifier: &mut Option<Modifier>,
    current_content: &mut Vec<String>,
    negated: &mut bool,
) {
    if let Some(modifier) = current_modifier {
        groups.push(ModifierGroup {
            modifier: *modifier,
            content: current_content.join(" "),
            negated: *negated,
        });

        *negated = false;
    }

    *current_modifier = None;
    *current_content = Vec::new();
}

pub async fn reaction_or_msg(ctx: PrefixContext<'_>, msg: &str, reaction: &str) {
    message_react(ctx, true, msg, reaction).await;
}

pub async fn msg_or_reaction(ctx: PrefixContext<'_>, msg: &str, reaction: &str) {
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
pub fn commands() -> [crate::Command; 2] {
    [purge(), purge_in()]
}
