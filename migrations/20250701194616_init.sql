CREATE TABLE guilds (
    id SERIAL PRIMARY KEY,
    guild_id BIGINT UNIQUE NOT NULL
);

CREATE TABLE channels (
    id SERIAL PRIMARY KEY,
    channel_id BIGINT UNIQUE NOT NULL,
    guild_id BIGINT,
    FOREIGN KEY (guild_id) REFERENCES guilds(id) ON DELETE CASCADE
);

CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    user_id BIGINT UNIQUE NOT NULL,
    -- no longer allowed to use any commands
    is_bot_banned BOOLEAN DEFAULT FALSE NOT NULL,
    -- allowed to use commands marked as such
    is_bot_admin BOOLEAN DEFAULT FALSE NOT NULL,
    -- allowed to use specific admin commands
    allowed_admin_commands TEXT[],
);

CREATE TYPE CommandType AS ENUM ('prefix', 'application');

CREATE TABLE executed_commands (
    id SERIAL PRIMARY KEY,
    user_id INT NOT NULL REFERENCES users(id),
    channel_id INT NOT NULL REFERENCES channels(id),
    guild_id INT REFERENCES guilds(id),
    command TEXT NOT NULL,
    command_type CommandType NOT NULL,
    executed_at TIMESTAMPZ NOT NULL,
    executed_successfully BOOLEAN NOT NULL,
    error_text TEXT
)

CREATE TABLE audit_log (
    audit_log_id BIGINT PRIMARY KEY,
    guild_id BIGINT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
    target_id BIGINT,
    action_kind SMALLINT NOT NULL,
    reason TEXT,
    user_id INT NOT NULL REFERENCES users(id),
    change JSONB,
    options JSONB,
    created_at TIMESTAMPTZ
);


CREATE TABLE role_snapshots (
    id BIGSERIAL PRIMARY KEY,
    user_id INT NOT NULL REFERENCES users(id),
    guild_id INT NOT NULL REFERENCES guilds(id)
    roles BIGINT[],
    snapshot_taken TIMESTAMPZ
)

CREATE TABLE messages (
    id BIGSERIAL PRIMARY KEY,
    message_id BIGINT UNIQUE NOT NULL,
    channel_id INT NOT NULL,
    user_id INT NOT NULL,
    guild_id INT,
    FOREIGN KEY (channel_id) REFERENCES channels(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (guild_id) REFERENCES guilds(id) ON DELETE SET NULL
);

CREATE TABLE dm_activity (
    user_id INT PRIMARY KEY REFERENCES users(id),
    last_announced TIMESTAMPZ,
    until TIMESTAMPZ,
    count SMALLINT
);


CREATE INDEX idx_messages_guild_id ON messages(guild_id);
CREATE INDEX idx_messages_channel_id ON messages(channel_id);

CREATE TYPE EmoteUsageType AS ENUM ('message', 'reaction');

CREATE TABLE stickers (
    sticker_id BIGINT PRIMARY KEY,
    sticker_name TEXT NOT NULL
);

CREATE TABLE sticker_usage (
    id SERIAL PRIMARY KEY,
    message_id BIGINT NOT NULL,
    user_id INT NOT NULL,
    sticker_id BIGINT NOT NULL,
    FOREIGN KEY (message_id) REFERENCES messages(id),
    FOREIGN KEY (user_id) REFERENCES users(id),
    FOREIGN KEY (sticker_id) REFERENCES stickers(sticker_id)
);

CREATE INDEX idx_user_id_sticker_usage ON sticker_usage(user_id);

CREATE TABLE emotes (
    id SERIAL PRIMARY KEY,
    emote_name TEXT NOT NULL,
    discord_id BIGINT UNIQUE
);

CREATE INDEX idx_discord_id ON emotes(discord_id);
CREATE INDEX idx_emote_name_discord_id ON emotes(emote_name, discord_id);

CREATE UNIQUE INDEX emote_name_discord_id_unique
    ON emotes (emote_name, discord_id);

CREATE UNIQUE INDEX emote_name_null_discord_id_unique
    ON emotes (emote_name)
    WHERE discord_id IS NULL;

CREATE UNIQUE INDEX unique_user_message_emote_reaction
    ON emote_usage (message_id, user_id, emote_id)
    WHERE usage_type = 'reaction';

CREATE TABLE emote_usage (
    id SERIAL PRIMARY KEY,
    message_id BIGINT NOT NULL,
    emote_id INT NOT NULL,
    user_id INT NOT NULL,
    used_at BIGINT NOT NULL,
    usage_type EmoteUsageType NOT NULL,
    FOREIGN KEY (message_id) REFERENCES messages(id),
    FOREIGN KEY (emote_id) REFERENCES emotes(id),
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE blocked_checked_emotes (
    guild_id INT NOT NULL REFERENCES guilds(id),
    emote_id INT NOT NULL,
    PRIMARY KEY (guild_id, emote_id),
    FOREIGN KEY (emote_id) REFERENCES emotes(id)
);

CREATE TABLE blocked_checked_stickers (
    guild_id INT NOT NULL REFERENCES guilds(id),
    sticker_id BIGINT NOT NULL,
    PRIMARY KEY (guild_id, sticker_id),
    FOREIGN KEY (sticker_id) REFERENCES stickers(sticker_id)
);


CREATE INDEX idx_user_id_emote ON emote_usage(user_id);
CREATE INDEX idx_message_id_emote ON emote_usage(message_id);

CREATE TYPE starboard_status AS ENUM ('InReview', 'Denied', 'Accepted');

CREATE TABLE starboard (
    id SERIAL PRIMARY KEY,
    user_id INT NOT NULL,
    username VARCHAR(32) NOT NULL,
    avatar_url TEXT,

    content TEXT NOT NULL,
    message_id BIGINT NOT NULL,
    attachment_urls TEXT[] NOT NULL,

    star_count SMALLINT NOT NULL,
    starboard_status starboard_status DEFAULT 'InReview' NOT NULL,
    starboard_message_id BIGINT NOT NULL,
    starboard_message_channel INT NOT NULL,

    -- special metadata
    forwarded BOOLEAN DEFAULT FALSE NOT NULL,
    reply_message_id BIGINT,
    reply_username VARCHAR(32),

    FOREIGN KEY (user_id) REFERENCES users(id),
    FOREIGN KEY (message_id) REFERENCES messages(id),
    FOREIGN KEY (starboard_message_channel) REFERENCES channels(id),
    FOREIGN KEY (reply_message_id) REFERENCES messages(id)
);


CREATE TABLE starboard_overrides(
    channel_id INT NOT NULL PRIMARY KEY,
    star_count SMALLINT NOT NULL,
    FOREIGN KEY (channel_id) REFERENCES channels(id)
)


CREATE TABLE regexes (
    id SERIAL PRIMARY KEY,
    guild_id BIGINT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
    -- channel or global reference
    channel_id INT REFERENCES channels(id) ON DELETE CASCADE,
    pattern TEXT NOT NULL,
    recurse_channels BOOLEAN NOT NULL DEFAULT TRUE,
    recurse_threads BOOLEAN NOT NULL DEFAULT TRUE,
    -- bitflags for the methods of detection.
    detection_type SMALLINT NOT NULL DEFAULT 1
);


CREATE TABLE regex_exceptions (
    regex_id INT NOT NULL REFERENCES regexes(id) ON DELETE CASCADE,
    channel_id BIGINT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    PRIMARY KEY (regex_id, channel_id)
);

CREATE TABLE responses (
    id SERIAL PRIMARY KEY,
    regex_id INT NOT NULL REFERENCES regexes(id) ON DELETE CASCADE,
    content TEXT,
    emote_id INT REFERENCES emotes(id) ON DELETE CASCADE,
    -- there must be at least *something* to respond with.
    CHECK (
      message IS NOT NULL OR emote_id IS NOT NULL
    )
);

CREATE TABLE verified_users(
    user_id INT NOT NULL PRIMARY KEY REFERENCES users(id),
    osu_id INT NOT NULL,
    last_updated BIGINT NOT NULL,
    is_active BOOLEAN NOT NULL,
    gamemode SMALLINT NOT NULL,
    rank INT,
    map_status SMALLINT,
    verified_roles BIGINT[]
);

CREATE TABLE transcendent_roles (
    id SMALLSERIAL PRIMARY KEY,
    user_id INT NOT NULL REFERENCES users(id),
    role_id BIGINT NOT NULL,
    UNIQUE (user_id, role_id)
);

