use ::serenity::all::{
    CreateAllowedMentions, CreateAttachment, CreateComponent, CreateTextDisplay, GenericChannelId,
    MessageFlags,
};
use lumi::{
    serenity_prelude::{
        self as serenity, Attachment, ChunkGuildFilter, Message, ReactionType, StickerId, UserId,
    },
    CreateReply,
};
use reqwest::{Client, Method};
use serde_json::{Map, Value};

use std::fmt::Write;

use crate::{owner::admin, Context, Error};

#[lumi::command(
    prefix_command,
    aliases("kys"),
    category = "Owner",
    owners_only,
    hide_in_help,
    has_modifier
)]
pub async fn shutdown(ctx: crate::PrefixContext<'_>) -> Result<(), Error> {
    if !ctx.mod_chars.contains('!') {
        ctx.say("**Bailing out, you are on your own. Good luck.**")
            .await?;
    }

    ctx.serenity_context().shutdown_all();

    Ok(())
}

/// Say something!
#[lumi::command(
    prefix_command,
    hide_in_help,
    check = "admin",
    category = "Admin - Say"
)]
pub async fn say(
    ctx: Context<'_>,
    #[description = "Channel where the message will be sent"] channel: Option<GenericChannelId>,
    #[description = "What to say"]
    #[rest]
    string: String,
) -> Result<(), Error> {
    let target_channel = channel.unwrap_or(ctx.channel_id());

    target_channel.say(ctx.http(), string).await?;

    Ok(())
}

// TODO: allow toggle of the replied user ping, also defer when attachment.

/// Say something in a specific channel.
///
/// Allowed mentions by default are set to true.
#[allow(clippy::too_many_arguments)]
#[lumi::command(slash_command, hide_in_help, check = "admin", category = "Admin - Say")]
pub async fn say_slash(
    ctx: Context<'_>,
    // Have to manually parse this because discord guild command.
    // Also doesn't let u64 just work??
    #[description = "Channel where the message will be sent"] channel: String,
    #[description = "What to say"] content: Option<String>,
    // parsed as a String and will be split later.
    #[description = "stickers (up to 3)"] sticker: Option<String>,
    #[description = "reply to?"] reply: Option<Message>,
    #[description = "attachment (limited to 1)"] attachment: Option<Attachment>,
    #[description = "Allow everyone ping?"] allow_everyone: Option<bool>,
    #[description = "Allow roles?"] allow_roles: Option<bool>,
    #[description = "Allow users?"] allow_users: Option<bool>,
) -> Result<(), Error> {
    ctx.defer().await?;

    let mut am = serenity::CreateAllowedMentions::new()
        .all_roles(true)
        .all_users(true)
        .everyone(true);

    if let Some(b) = allow_everyone {
        am = am.everyone(b);
    }

    if let Some(b) = allow_roles {
        am = am.all_roles(b);
    }

    if let Some(b) = allow_users {
        am = am.all_users(b);
    }

    let mut b = serenity::CreateMessage::new().allowed_mentions(am);

    if let Some(content) = content {
        b = b.content(content);
    }

    // Overhall this later, because allocations.
    if let Some(sticker) = sticker {
        let stickers: Vec<_> = sticker.split(", ").collect();

        // Will panic if it can't be parsed, future me issue.
        let sticker_ids: Vec<StickerId> = stickers
            .iter()
            .map(|s| StickerId::new(s.parse().unwrap()))
            .collect();

        b = b.add_sticker_ids(sticker_ids);
    }

    if let Some(reply) = reply {
        b = b.reference_message(&reply);
    }

    if let Some(attachment) = attachment {
        b = b.add_file(serenity::CreateAttachment::bytes(
            attachment.download().await?,
            attachment.filename,
        ));
    }

    let result = GenericChannelId::new(channel.parse::<u64>().unwrap())
        .send_message(ctx.http(), b)
        .await;

    // Respond to the slash command.
    match result {
        Ok(_) => ctx.say("Successfully sent message!").await?,
        Err(err) => ctx.say(format!("{err}")).await?,
    };

    Ok(())
}

/// dm a user!
#[lumi::command(
    prefix_command,
    hide_in_help,
    category = "Admin - Say",
    check = "admin"
)]
pub async fn dm(
    ctx: Context<'_>,
    #[description = "ID"] user_id: UserId,
    #[rest]
    #[description = "Message"]
    message: String,
) -> Result<(), Error> {
    user_id
        .dm(
            ctx.http(),
            serenity::CreateMessage::default().content(message),
        )
        .await?;

    Ok(())
}

/// React to a message with a specific reaction!
#[lumi::command(
    prefix_command,
    hide_in_help,
    category = "Admin - Messages",
    check = "admin"
)]
pub async fn react(
    ctx: Context<'_>,
    #[description = "Message to react to"] message: Message,
    #[description = "What to React with"] string: String,
) -> Result<(), Error> {
    // dumb stuff to get around discord stupidly attempting to strip the parsing.
    let trimmed_string = string.trim_matches('`').trim_matches('\\').to_string();
    // React to the message with the specified emoji
    let reaction = trimmed_string.parse::<ReactionType>().unwrap(); // You may want to handle parsing errors
    message.react(ctx.http(), reaction).await?;

    Ok(())
}

// This halfs the memory usage at startup, not sure about other cases.
#[lumi::command(prefix_command, category = "Owner", owners_only, hide_in_help)]
async fn malloc_trim(ctx: Context<'_>) -> Result<(), Error> {
    unsafe {
        libc::malloc_trim(0);
    }

    ctx.say("Trimmed.").await?;

    Ok(())
}

/// requests chunks of all guild members in the current guild.
#[lumi::command(
    rename = "chunk-guild-members",
    prefix_command,
    check = "admin",
    category = "Admin - Cache",
    hide_in_help,
    guild_only
)]
async fn chunk_guild_members(ctx: Context<'_>, presences: Option<bool>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    ctx.serenity_context().chunk_guild(
        guild_id,
        None,
        presences.unwrap_or(false),
        ChunkGuildFilter::None,
        None,
    );

    ctx.say("Requesting guild member chunks").await?;

    Ok(())
}

#[lumi::command(
    rename = "fw-commands",
    prefix_command,
    check = "admin",
    category = "Admin - Commands",
    hide_in_help,
    guild_only
)]
async fn fw_commands(ctx: Context<'_>) -> Result<(), Error> {
    let commands = &ctx.framework().options.commands;

    for command in commands {
        if command.aliases.is_empty() {
            println!("{}", command.name);
        } else {
            println!("{}: {:?}", command.name, command.aliases);
        }
    }

    Ok(())
}

#[lumi::command(prefix_command, owners_only, hide_in_help, guild_only)]
async fn sudo(
    ctx: lumi::PrefixContext<'_, crate::Data, Error>,
    user: serenity::User,
    #[rest] rest: String,
) -> Result<(), Error> {
    let mut msg = ctx.msg.clone();
    // set member, if available.
    if let Some(guild_id) = ctx.guild_id() {
        if let Ok(member) = guild_id.member(ctx.http(), user.id).await {
            msg.member = Some(std::boxed::Box::new(member.into()));
        } else {
            msg.member = None;
        }
    }

    // set user.
    msg.author = user;

    // There is about 1000 ways to do this that are better but...
    // I don't care!
    let content = format!("-{rest}");
    msg.content = small_fixed_array::FixedString::from_string_trunc(content);

    if let Err(err) = lumi::dispatch_message(
        ctx.framework,
        &msg,
        lumi::MessageDispatchTrigger::MessageCreate,
        &tokio::sync::Mutex::new(std::boxed::Box::new(()) as _),
        &mut Vec::new(),
    )
    .await
    {
        err.handle(ctx.framework.options).await;
    }

    Ok(())
}

#[lumi::command(
    prefix_command,
    check = "admin",
    category = "Admin - Commands",
    hide_in_help,
    guild_only
)]
async fn analyze(ctx: Context<'_>, #[rest] msg: String) -> Result<(), Error> {
    let kind = format!("{:?}", moth_filter::analyze(&msg));
    ctx.say(kind).await?;
    Ok(())
}

#[lumi::command(
    rename = "members-dump",
    prefix_command,
    check = "admin",
    category = "Admin - Commands",
    hide_in_help,
    guild_only
)]
async fn members_dump(ctx: Context<'_>, full: Option<bool>) -> Result<(), Error> {
    let full = full.unwrap_or(false);

    let mut writer = String::new();

    {
        let Some(guild) = ctx.guild() else {
            ctx.say("guild is not cached.").await?;
            return Ok(());
        };

        for member in &guild.members {
            if full {
                writeln!(writer, "{member:?}").unwrap();
            } else {
                writeln!(
                    writer,
                    "{}: {:?}: {:?} (ID:{})",
                    member.user.name, member.user.global_name, member.nick, member.user.id
                )
                .unwrap();
            }
        }
    }

    ctx.send(CreateReply::new().attachment(CreateAttachment::bytes(writer, "members.txt")))
        .await?;

    Ok(())
}

// TODO: parse this using proper arguments
#[lumi::command(
    prefix_command,
    owners_only,
    category = "Admin - Commands",
    hide_in_help
)]
async fn http(ctx: Context<'_>, #[rest] input: String) -> Result<(), Error> {
    // Split off everything after the first | (if any)
    let split: Vec<&str> = input.splitn(2, '|').map(str::trim).collect();
    let (method_and_route, rest) = match split.as_slice() {
        [a] => (*a, ""),
        [a, b] => (*a, *b),
        _ => {
            ctx.say("❌ Invalid input format").await?;
            return Ok(());
        }
    };

    let mut parts = method_and_route.split_whitespace();
    let method_str = if let Some(m) = parts.next() {
        m.to_uppercase()
    } else {
        ctx.say("❌ Missing HTTP method").await?;
        return Ok(());
    };
    let Some(route) = parts.next() else {
        ctx.say("❌ Missing route").await?;
        return Ok(());
    };

    // Split optional body and headers
    let mut rest_parts = rest.splitn(2, '|').map(str::trim);
    let body_json = rest_parts.next().unwrap_or("");
    let headers_json = rest_parts.next().unwrap_or("");

    let method = match method_str.as_str() {
        "GET" => Method::GET,
        "POST" => Method::POST,
        "PATCH" => Method::PATCH,
        "DELETE" => Method::DELETE,
        "PUT" => Method::PUT,
        _ => {
            ctx.say("❌ Invalid HTTP method").await?;
            return Ok(());
        }
    };

    let url = format!("https://discord.com/api/v10{route}");
    let token = std::env::var("MOTH_TOKEN").expect("MOTH_TOKEN must be set");

    let client = Client::new();
    let mut request = client
        .request(method, &url)
        .header("Authorization", format!("Bot {token}"))
        .header("Content-Type", "application/json");

    // Add headers if any
    if !headers_json.is_empty() {
        match serde_json::from_str::<Map<String, Value>>(headers_json) {
            Ok(map) => {
                for (k, v) in map {
                    if let Some(v_str) = v.as_str() {
                        request = request.header(k, v_str);
                    }
                }
            }
            Err(e) => {
                ctx.say(format!("❌ Invalid headers JSON: {e}")).await?;
                return Ok(());
            }
        }
    }

    // Add body if any
    if !body_json.is_empty() {
        match serde_json::from_str::<Value>(body_json) {
            Ok(json) => request = request.json(&json),
            Err(e) => {
                ctx.say(format!("❌ Invalid JSON body: {e}")).await?;
                return Ok(());
            }
        }
    }

    let response = match request.send().await {
        Ok(r) => r,
        Err(e) => {
            ctx.say(format!("❌ HTTP request failed: {e}")).await?;
            return Ok(());
        }
    };

    let status = response.status();
    let text = match response.text().await {
        Ok(raw) => match serde_json::from_str::<serde_json::Value>(&raw) {
            Ok(json) => {
                if raw.is_empty() {
                    String::from("{}")
                } else {
                    serde_json::to_string_pretty(&json).unwrap_or(raw)
                }
            }
            Err(_) => raw,
        },
        Err(_) => "<no body>".into(),
    };

    let mut builder = CreateReply::new().allowed_mentions(CreateAllowedMentions::new());

    let content = format!("**{method_str}** `{route}`\n**Status:** {status}\n```json\n{text}```");
    if content.len() > 4000 {
        ctx.send(
            builder
                .content(format!("**{method_str}** `{route}`\n**Status:** {status}"))
                .attachment(CreateAttachment::bytes(text.into_bytes(), "output.json")),
        )
        .await?;
    } else {
        let components = [CreateComponent::TextDisplay(CreateTextDisplay::new(
            content,
        ))];
        builder = builder
            .components(&components)
            .flags(MessageFlags::IS_COMPONENTS_V2);
        ctx.send(builder).await?;
    }

    Ok(())
}

#[must_use]
pub fn commands() -> [crate::Command; 11] {
    let say = lumi::Command {
        slash_action: say_slash().slash_action,
        parameters: say_slash().parameters,
        ..say()
    };

    [
        shutdown(),
        say,
        dm(),
        react(),
        malloc_trim(),
        chunk_guild_members(),
        fw_commands(),
        sudo(),
        analyze(),
        members_dump(),
        http(),
    ]
}
