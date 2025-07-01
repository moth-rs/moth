use rosu_v2::{
    model::GameMode,
    prelude::{RankStatus, UserExtended},
    request::MapType,
};
use serenity::all::{
    CreateEmbed, CreateEmbedAuthor, CreateMessage, EditMember, GenericChannelId, GuildId,
    MessageFlags, RoleId, UserId,
};

use crate::data::structs::Data;

struct RoleRange {
    min_rank: u32,
    max_rank: u32,
    role_id: RoleId,
}

const GUILD_ID: GuildId = GuildId::new(98226572468690944);
pub const LOG_CHANNEL: GenericChannelId = GenericChannelId::new(776522946872344586);

#[rustfmt::skip]
const OSU_RANGES: &[RoleRange] = &[
    RoleRange { min_rank: 1, max_rank: 99, role_id: RoleId::new(754085973003993119) },
    RoleRange { min_rank: 100, max_rank: 499, role_id: RoleId::new(754086188025118770) },
    RoleRange { min_rank: 500, max_rank: 999, role_id: RoleId::new(754086290785304627) },
    RoleRange { min_rank: 1000, max_rank: 4999, role_id: RoleId::new(754086299681685696) },
    RoleRange { min_rank: 5000, max_rank: 9999, role_id: RoleId::new(869294796404035675) },
    RoleRange { min_rank: 10000, max_rank: 24999, role_id: RoleId::new(869295190601531462) },
    RoleRange { min_rank: 25000, max_rank: 49999, role_id: RoleId::new(869295555489202217) },
    RoleRange { min_rank: 50000, max_rank: 99999, role_id: RoleId::new(754086107456471062) },
    RoleRange { min_rank: 100000, max_rank: 499999, role_id: RoleId::new(754089529287245855) },
    RoleRange { min_rank: 500000, max_rank: u32::MAX, role_id: RoleId::new(869295874306605066) },
];

#[rustfmt::skip]
const MANIA_RANGES: &[RoleRange] = &[
    RoleRange { min_rank: 1, max_rank: 99, role_id: RoleId::new(754086656889585714) },
    RoleRange { min_rank: 100, max_rank: 499, role_id: RoleId::new(754086784484376596) },
    RoleRange { min_rank: 500, max_rank: 999, role_id: RoleId::new(754086852524507246) },
    RoleRange { min_rank: 1000, max_rank: 4999, role_id: RoleId::new(754086905825460265) },
    RoleRange { min_rank: 5000, max_rank: 9999, role_id: RoleId::new(754086720638681109) },
    RoleRange { min_rank: 10000, max_rank: 24999, role_id: RoleId::new(754089662242357289) },
    RoleRange { min_rank: 25000, max_rank: 49999, role_id: RoleId::new(869296510909689896) },
    RoleRange { min_rank: 50000, max_rank: 99999, role_id: RoleId::new(869296562881302528) },
    RoleRange { min_rank: 100000, max_rank: 499999, role_id: RoleId::new(869296602869801070) },
    RoleRange { min_rank: 500000, max_rank: u32::MAX, role_id: RoleId::new(869296657882300446) },
];

#[rustfmt::skip]
const TAIKO_RANGES: &[RoleRange] = &[
    RoleRange { min_rank: 1, max_rank: 99, role_id: RoleId::new(754087013904547930) },
    RoleRange { min_rank: 100, max_rank: 499, role_id: RoleId::new(754087748209475595) },
    RoleRange { min_rank: 500, max_rank: 999, role_id: RoleId::new(754087814106448012) },
    RoleRange { min_rank: 1000, max_rank: 4999, role_id: RoleId::new(754087911066173460) },
    RoleRange { min_rank: 5000, max_rank: 9999, role_id: RoleId::new(754087679003721790) },
    RoleRange { min_rank: 10000, max_rank: 24999, role_id: RoleId::new(754089750717136906) },
    RoleRange { min_rank: 25000, max_rank: 49999, role_id: RoleId::new(869297047050784870) },
    RoleRange { min_rank: 50000, max_rank: 99999, role_id: RoleId::new(869297101086011483 )},
    RoleRange { min_rank: 100000, max_rank: 499999, role_id: RoleId::new(869297132958531584) },
    RoleRange { min_rank: 500000, max_rank: u32::MAX, role_id: RoleId::new(869297154253017108) },
];

#[rustfmt::skip]
const CTB_RANGES: &[RoleRange] = &[
    RoleRange { min_rank: 1, max_rank: 99, role_id: RoleId::new(754087989717762080) },
    RoleRange { min_rank: 100, max_rank: 499, role_id: RoleId::new(754088203534729276) },
    RoleRange { min_rank: 500, max_rank: 999, role_id: RoleId::new(754088281674743858) },
    RoleRange { min_rank: 1000, max_rank: 4999, role_id: RoleId::new(754088358916915241) },
    RoleRange { min_rank: 5000, max_rank: 9999, role_id: RoleId::new(754088053101953034) },
    RoleRange { min_rank: 10000, max_rank: 24999, role_id: RoleId::new(754089875157942435) },
    RoleRange { min_rank: 25000, max_rank: 49999, role_id: RoleId::new(869299174556987403) },
    RoleRange { min_rank: 50000, max_rank: 99999, role_id: RoleId::new(869299210883850280) },
    RoleRange { min_rank: 100000, max_rank: 499999, role_id: RoleId::new(869299235592478770) },
    RoleRange { min_rank: 500000, max_rank: u32::MAX, role_id: RoleId::new(869299254076792892) },
];

const ALL_RANGES: [&[RoleRange]; 4] = [OSU_RANGES, MANIA_RANGES, TAIKO_RANGES, CTB_RANGES];

fn get_role_id_for_rank(game_mode: GameMode, rank: u32) -> RoleId {
    match game_mode {
        GameMode::Osu => find_role_for_rank(OSU_RANGES, rank),
        GameMode::Mania => find_role_for_rank(MANIA_RANGES, rank),
        GameMode::Taiko => find_role_for_rank(TAIKO_RANGES, rank),
        GameMode::Catch => find_role_for_rank(CTB_RANGES, rank),
    }
}

#[must_use]
pub fn get_role_id_for_rank_opt(game_mode: GameMode, rank: Option<u32>) -> Option<RoleId> {
    Some(get_role_id_for_rank(game_mode, rank?))
}

fn find_role_for_rank(ranges: &[RoleRange], rank: u32) -> RoleId {
    // Simple linear search, which should be fine since the data is small
    ranges
        .iter()
        .find(|range| rank >= range.min_rank && rank <= range.max_rank)
        .map(|range| range.role_id)
        .expect("All ranges have a u32::MAX")
}

const SPECIAL_MAPPING: &[(u32, RoleId)] = &[
    // GMT
    (4, RoleId::new(974674488803340338)),
    // PROJECT LOVED
    (31, RoleId::new(969880026084429824)),
    // FEATURED ARTIST
    (35, RoleId::new(901768871038570546)),
    // NOMINATION ASSESSMENT TEAM
    (7, RoleId::new(1069665975630315611)),
    // BEATMAP SPOTLIGHT CURATOR
    (48, RoleId::new(1089591328985329716)),
    // BEATMAP NOMINATOR
    (28, RoleId::new(901772287445987348)),
    // ALM is 16, but not yet supported as not needed.
];

#[expect(clippy::type_complexity)]
const ALL_MAPPER_ROLES: &[(fn(&UserMapHolder) -> bool, RoleId)] = &[
    // Ranked roles
    (|u| u.ranked_std(), RoleId::new(1041039012179222660)),
    (|u| u.ranked_mania(), RoleId::new(1041036116482080811)),
    (|u| u.ranked_taiko(), RoleId::new(1041036580770562149)),
    (|u| u.ranked_catch(), RoleId::new(1041036816909881404)),
    // Loved roles
    (|u| u.loved_std(), RoleId::new(1056525314303475752)),
    (|u| u.loved_mania(), RoleId::new(1120351610107858985)),
    (|u| u.loved_taiko(), RoleId::new(1120351662075289641)),
    (|u| u.loved_catch(), RoleId::new(1120351771634712646)),
];

#[bool_to_bitflags::bool_to_bitflags]
#[derive(Default, Eq, PartialEq, Copy, Clone)]
pub struct UserMapHolder {
    // DO NOT change the order of these.
    // well, its not a big deal but the bot will recalc everyone due to different bits.
    ranked_std: bool,
    ranked_mania: bool,
    ranked_taiko: bool,
    ranked_catch: bool,
    loved_std: bool,
    loved_mania: bool,
    loved_taiko: bool,
    loved_catch: bool,
}

impl UserMapHolder {
    /// Returns the set of **all possible** mapper roles this struct knows about
    pub fn all_roles() -> impl Iterator<Item = RoleId> {
        ALL_MAPPER_ROLES.iter().map(|(_, role)| *role)
    }

    /// Returns the set of roles this user **should** have based on the flags
    pub fn active_roles(&self) -> impl Iterator<Item = RoleId> + '_ {
        ALL_MAPPER_ROLES
            .iter()
            .filter_map(move |(check, role)| if check(self) { Some(*role) } else { None })
    }

    #[must_use]
    pub fn bits(&self) -> u8 {
        self.__generated_flags.bits()
    }

    #[must_use]
    pub fn from_bits(bits: u8) -> Self {
        Self {
            __generated_flags: UserMapHolderGeneratedFlags::from_bits(bits)
                .expect("should not be provided with invalid bits."),
        }
    }
}

#[expect(clippy::too_many_lines)]
pub async fn update_roles(
    ctx: &serenity::all::Context,
    user_id: UserId,
    user: Option<&UserExtended>,
    metadata: Option<MetadataType<'_>>,
    reason: &str,
) -> bool {
    // unlink -> remove everything.
    let (Some(user), Some(metadata)) = (user, metadata) else {
        kill_roles(ctx, user_id).await;
        return true;
    };

    let groups = user.groups.as_deref().expect("always sent");

    // we do the osu map checking up here instead of below to minimise the amount of time we are using "outdated" roles.
    // if we wait too long the chances of say, chirou muting them or a mute expiring increases.
    // if we do it here we will only wait a couple micros at most.
    let mut holder = UserMapHolder::default();
    if user.guest_mapset_count.expect("always sent") > 0 {
        handle_maps(ctx, user.user_id, MapTypeChoice::GuestEither, &mut holder).await;
    }
    if user.ranked_mapset_count.expect("always sent") > 0 {
        handle_maps(ctx, user.user_id, MapTypeChoice::Ranked, &mut holder).await;
    }
    if user.loved_mapset_count.expect("always sent") > 0 {
        handle_maps(ctx, user.user_id, MapTypeChoice::Loved, &mut holder).await;
    }

    let current_rank = user.statistics.as_ref().expect("always sent").global_rank;
    let matched_roles = SPECIAL_MAPPING
        .iter()
        .filter(|(id, _)| groups.iter().any(|g| g.id == *id))
        .map(|(_, role)| *role)
        .collect::<Vec<_>>();

    // basically, if any condition is not equal, we recalc.
    let is_outdated = metadata.initial_verification()
        || holder != metadata.mapping_or_default()
        || !metadata
            .verified_roles_or_default()
            .eq(matched_roles.iter().copied())
        || get_role_id_for_rank_opt(metadata.gamemode(), metadata.rank())
            != get_role_id_for_rank_opt(metadata.gamemode(), current_rank);

    if !is_outdated {
        return true;
    }

    let Ok(member) = ctx.http.get_member(GUILD_ID, user_id).await else {
        println!("could not fetch member, failing...");
        return false;
    };

    let mut roles = member.roles.to_vec();

    // remove existing rank roles from the users roles.
    roles.retain(|role_id| {
        !ALL_RANGES
            .iter()
            .flat_map(|slice| slice.iter())
            .any(|range| *role_id == range.role_id)
    });

    roles.retain(|role_id| {
        // Check if the role is a known special role
        !SPECIAL_MAPPING.iter().any(|(_, r)| r == role_id)
    });

    roles.retain(|role_id| {
        // Remove role if it's in ALL_MAPPER_ROLES
        !UserMapHolder::all_roles().any(|r| r == *role_id)
    });

    // assign the special roles they should have.
    let mut new_special = Vec::new();
    let mut removed_special = Vec::new();
    for role_id in &matched_roles {
        // we use this to notify if we assigned a new special role.
        if !member.roles.contains(role_id) {
            new_special.push(*role_id);
        }
        roles.push(*role_id);
    }

    for active_role in holder.active_roles() {
        if !member.roles.contains(&active_role) {
            new_special.push(active_role);
        }
        roles.push(active_role);
    }

    // Conditionally add the new role (only for update, not remove)
    if let Some(rank) = current_rank {
        roles.push(get_role_id_for_rank(metadata.gamemode(), rank));
    }

    // populate the removed_special variable
    for (_, role_id) in SPECIAL_MAPPING {
        if member.roles.contains(role_id) && !roles.contains(role_id) {
            removed_special.push(*role_id);
        }
    }

    for role_id in UserMapHolder::all_roles() {
        if member.roles.contains(&role_id) && !roles.contains(&role_id) {
            removed_special.push(role_id);
        }
    }

    if *roles == *member.roles {
        return true;
    }

    if GUILD_ID
        .edit_member(
            &ctx.http,
            user_id,
            EditMember::new().roles(roles).audit_log_reason(reason),
        )
        .await
        .is_err()
    {
        println!("failed to edit member...");
        return false;
    }

    let time = chrono::Utc::now();
    let _ = ctx
        .data::<Data>()
        .database
        .update_last_updated(
            user_id,
            time,
            Some(current_rank),
            holder.bits(),
            &matched_roles,
        )
        .await;

    let embed = CreateEmbed::new()
        .author(
            CreateEmbedAuthor::new(user.username.as_str())
                .url(format!("https://osu.ppy.sh/u/{}", user.user_id)),
        )
        .description("Assigned one or more special roles.")
        .field("Discord user", format!("<@{user_id}>"), true)
        .thumbnail(&user.avatar_url);

    for role in new_special {
        let embed = embed.clone().field("Role", format!("<@&{role}>"), true);

        let _ = LOG_CHANNEL
            .send_message(
                &ctx.http,
                CreateMessage::new()
                    // phil and me
                    .content("<@101090238067113984> <@158567567487795200>")
                    .embed(embed)
                    .flags(MessageFlags::SUPPRESS_NOTIFICATIONS),
            )
            .await;
    }

    let embed = CreateEmbed::new()
        .author(
            CreateEmbedAuthor::new(user.username.as_str())
                .url(format!("https://osu.ppy.sh/u/{}", user.user_id)),
        )
        .description("Removed one or more special roles.")
        .field("Discord user", format!("<@{user_id}>"), true)
        .thumbnail(&user.avatar_url);

    for role in removed_special {
        let embed = embed.clone().field("Role", format!("<@&{role}>"), true);

        let _ = LOG_CHANNEL
            .send_message(
                &ctx.http,
                CreateMessage::new()
                    // phil and me
                    .content("<@101090238067113984> <@291089948709486593> <@158567567487795200>")
                    .flags(MessageFlags::SUPPRESS_NOTIFICATIONS)
                    .embed(embed),
            )
            .await;
    }

    true
}

pub enum MetadataType<'a> {
    GameMode(GameMode),
    Full(&'a super::Metadata),
}

impl MetadataType<'_> {
    #[must_use]
    pub fn gamemode(&self) -> GameMode {
        match self {
            MetadataType::GameMode(game_mode) => *game_mode,
            MetadataType::Full(metadata) => metadata.gamemode,
        }
    }

    #[must_use]
    pub fn mapping_or_default(&self) -> UserMapHolder {
        match self {
            MetadataType::GameMode(_) => UserMapHolder::default(),
            MetadataType::Full(metadata) => metadata.map_status,
        }
    }

    pub fn verified_roles_or_default(&self) -> impl Iterator<Item = RoleId> + '_ {
        match self {
            MetadataType::GameMode(_) => [].iter(),
            MetadataType::Full(metadata) => metadata.verified_roles.iter(),
        }
        .copied()
    }

    #[must_use]
    pub fn rank(&self) -> Option<u32> {
        match self {
            MetadataType::GameMode(_) => None,
            MetadataType::Full(metadata) => metadata.rank,
        }
    }

    #[must_use]
    pub fn initial_verification(&self) -> bool {
        match self {
            // update or verification is considered "initial" and subject to full refresh.
            MetadataType::GameMode(_) => true,
            MetadataType::Full(metadata) => metadata.initial_verification,
        }
    }
}

#[derive(Copy, Clone)]
enum MapTypeChoice {
    Loved,
    Ranked,
    GuestEither,
}

impl From<MapTypeChoice> for MapType {
    fn from(val: MapTypeChoice) -> Self {
        match val {
            MapTypeChoice::Loved => MapType::Loved,
            MapTypeChoice::Ranked => MapType::Ranked,
            MapTypeChoice::GuestEither => MapType::Guest,
        }
    }
}

async fn handle_maps(
    ctx: &serenity::all::Context,
    user_id: u32,
    map_type: MapTypeChoice,
    holder: &mut UserMapHolder,
) {
    let osu = &ctx.data::<Data>().web.osu;
    let mut offset = 0;

    if holder.__generated_flags.is_all() {
        return;
    }

    loop {
        let Ok(mapsets) = osu
            .user_beatmapsets(user_id)
            .status(&map_type.into())
            .offset(offset)
            .limit(5)
            .await
        else {
            return;
        };

        let len = mapsets.len();
        offset += len;

        for mapset in mapsets {
            for map in mapset.maps.expect("always sent") {
                if map.creator_id == user_id {
                    match map.status {
                        RankStatus::Ranked | RankStatus::Approved => match map.mode {
                            GameMode::Osu => holder.set_ranked_std(true),
                            GameMode::Taiko => holder.set_ranked_taiko(true),
                            GameMode::Catch => holder.set_ranked_catch(true),
                            GameMode::Mania => holder.set_ranked_mania(true),
                        },
                        RankStatus::Loved => match map.mode {
                            GameMode::Osu => holder.set_loved_std(true),
                            GameMode::Taiko => holder.set_loved_taiko(true),
                            GameMode::Catch => holder.set_loved_catch(true),
                            GameMode::Mania => holder.set_loved_mania(true),
                        },
                        _ => {}
                    }
                }

                if holder.__generated_flags.is_all() {
                    break;
                }
            }
        }

        if len != 5 {
            break;
        }
    }
}

async fn kill_roles(ctx: &serenity::all::Context, user_id: UserId) {
    let Ok(mut member) = ctx.http.get_member(GUILD_ID, user_id).await else {
        return;
    };

    let mut roles = member.roles.to_vec();

    // remove existing rank roles from the users roles.
    roles.retain(|role_id| {
        !ALL_RANGES
            .iter()
            .flat_map(|slice| slice.iter())
            .any(|range| *role_id == range.role_id)
    });

    roles.retain(|role_id| {
        // Check if the role is a known special role
        !SPECIAL_MAPPING.iter().any(|(_, r)| r == role_id)
    });

    roles.retain(|role_id| {
        // Remove role if it's in ALL_MAPPER_ROLES
        !UserMapHolder::all_roles().any(|r| r == *role_id)
    });

    let _ = member
        .edit(
            &ctx.http,
            EditMember::new()
                .roles(roles)
                .audit_log_reason("Removing roles due to unverification."),
        )
        .await;
}

pub(super) async fn maybe_update(
    ctx: &serenity::all::Context,
    user_id: UserId,
    user: Option<&UserExtended>,
    metadata: Option<MetadataType<'_>>,
) -> bool {
    update_roles(
        ctx,
        user_id,
        user,
        metadata,
        "Roles adjusted due to osu! rank update.",
    )
    .await
}

pub async fn remove(
    ctx: &serenity::all::Context,
    user_id: UserId,
    user: Option<&UserExtended>,
) -> bool {
    update_roles(ctx, user_id, user, None, "User has unverified.").await
}
