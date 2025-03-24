use lumi::serenity_prelude::{
    self as serenity, Attachment, ChannelId, ChunkGuildFilter, Message, ReactionType, StickerId,
    UserId,
};

use crate::{owner::owner, Context, Error};

#[lumi::command(
    prefix_command,
    aliases("kys"),
    category = "Owner",
    owners_only,
    hide_in_help
)]
pub async fn shutdown(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("**Bailing out, you are on your own. Good luck.**")
        .await?;
    ctx.serenity_context().shutdown_all();
    Ok(())
}

/// Say something!
#[lumi::command(
    prefix_command,
    hide_in_help,
    check = "owner",
    category = "Owner - Say"
)]
pub async fn say(
    ctx: Context<'_>,
    #[description = "Channel where the message will be sent"] channel: Option<ChannelId>,
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
#[lumi::command(slash_command, hide_in_help, check = "owner", category = "Owner - Say")]
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

    let result = ChannelId::new(channel.parse::<u64>().unwrap())
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
    category = "Owner - Say",
    check = "owner"
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
    category = "Owner - Messages",
    check = "owner"
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
    check = "owner",
    category = "Owner - Cache",
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
    check = "owner",
    category = "Owner - Commands",
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
    check = "owner",
    category = "Owner - Commands",
    hide_in_help,
    guild_only
)]
async fn analyze(ctx: Context<'_>, #[rest] msg: String) -> Result<(), Error> {
    let kind = format!("{:?}", moth_filter::analyze(&msg));
    ctx.say(kind).await?;
    Ok(())
}

#[must_use]
pub fn commands() -> [crate::Command; 9] {
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
    ]
}
