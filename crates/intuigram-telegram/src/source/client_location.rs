use intuigram_lib::{GeoPointView, PlaceView};

use super::client_rich_media::InputMediaSend;
use super::*;

/// One static-coordinate submission.
pub struct StaticLocationSend {
    /// Destination Chat.
    pub chat: ChatId,

    /// Exact validated coordinate.
    pub point: GeoPointView,

    /// Direct reply target.
    pub reply_to: Option<MessageId>,

    /// Active Thread root.
    pub thread_root: Option<MessageId>,

    /// User topic inside an administrator-owned monoforum.
    pub monoforum_peer: Option<ChatId>,

    /// Stable Message idempotency identifier.
    pub random_id: i64,
}

/// One durable normalized venue submission.
pub struct VenueSend {
    /// Destination Chat.
    pub chat: ChatId,

    /// Complete normalized venue payload.
    pub venue: PlaceView,

    /// Direct reply target.
    pub reply_to: Option<MessageId>,

    /// Active Thread root.
    pub thread_root: Option<MessageId>,

    /// User topic inside an administrator-owned monoforum.
    pub monoforum_peer: Option<ChatId>,

    /// Stable Message idempotency identifier.
    pub random_id: i64,
}

impl Client {
    /// Searches Telegram's configured venue provider in the active Chat
    /// context.
    pub async fn search_places(
        &mut self,
        chat: ChatId,
        query: String,
        near: Option<GeoPointView>,
    ) -> Result<Vec<PlaceView>> {
        let bot = self.venue_search_bot().await?;
        let peer = self.peers.resolve(chat)?;
        let response = self
            .connection
            .invoke(&inline_results_request(bot, peer, query, near))
            .await
            .context(InvokeSnafu)?;
        let tl::enums::messages::BotResults::Results(results) = response;
        self.update_peer_cache(&[], &results.users);
        Ok(normalize_place_results(results.results))
    }

    /// Sends one explicit static coordinate through the ordinary reply
    /// pipeline.
    pub async fn send_static_location(&mut self, request: StaticLocationSend) -> Result<MessageId> {
        self.send_static_location_with_policy(request, InvocationPolicy::WaitForFlood)
            .await
    }

    /// Sends one static coordinate using the requested invocation policy.
    pub async fn send_static_location_with_policy(
        &mut self,
        request: StaticLocationSend,
        policy: InvocationPolicy,
    ) -> Result<MessageId> {
        let peer = self.peers.resolve(request.chat)?;
        self.send_input_media_with_policy(
            InputMediaSend {
                peer,
                media: location_media(request.point),
                message: String::new(),
                reply_to: request.reply_to,
                thread_root: request.thread_root,
                monoforum_peer: request.monoforum_peer,
                random_id: request.random_id,
            },
            policy,
        )
        .await
    }

    /// Sends one normalized venue without relying on an ephemeral inline result
    /// ID.
    pub async fn send_venue(&mut self, request: VenueSend) -> Result<MessageId> {
        self.send_venue_with_policy(request, InvocationPolicy::WaitForFlood)
            .await
    }

    /// Sends one venue using the requested invocation policy.
    pub async fn send_venue_with_policy(
        &mut self,
        request: VenueSend,
        policy: InvocationPolicy,
    ) -> Result<MessageId> {
        let peer = self.peers.resolve(request.chat)?;
        self.send_input_media_with_policy(
            InputMediaSend {
                peer,
                media: venue_media(request.venue),
                message: String::new(),
                reply_to: request.reply_to,
                thread_root: request.thread_root,
                monoforum_peer: request.monoforum_peer,
                random_id: request.random_id,
            },
            policy,
        )
        .await
    }

    async fn venue_search_bot(&mut self) -> Result<tl::enums::InputUser> {
        if let Some(bot) = &self.venue_search_bot {
            return Ok(bot.clone());
        }
        let username = self
            .venue_search_username
            .clone()
            .context(VenueSearchUnavailableSnafu)?;
        let response = self
            .connection
            .invoke(&tl::functions::contacts::ResolveUsername {
                username,
                referer: None,
            })
            .await
            .context(InvokeSnafu)?;
        let tl::enums::contacts::ResolvedPeer::Peer(resolved) = response;
        let resolved_id = match resolved.peer {
            tl::enums::Peer::User(user) => user.user_id,
            _ => return VenueSearchBotUnavailableSnafu.fail(),
        };
        let bot = resolved_venue_bot(resolved_id, &resolved.users);
        self.update_peer_cache(&resolved.chats, &resolved.users);
        let bot = bot.context(VenueSearchBotUnavailableSnafu)?;
        self.venue_search_bot = Some(bot.clone());
        Ok(bot)
    }
}

pub(super) fn inline_results_request(
    bot: tl::enums::InputUser,
    peer: tl::enums::InputPeer,
    query: String,
    near: Option<GeoPointView>,
) -> tl::functions::messages::GetInlineBotResults {
    tl::functions::messages::GetInlineBotResults {
        bot,
        peer,
        geo_point: near.map(input_geo_point),
        query,
        offset: String::new(),
    }
}

pub(super) fn location_media(point: GeoPointView) -> tl::enums::InputMedia {
    tl::types::InputMediaGeoPoint {
        geo_point: input_geo_point(point),
    }
    .into()
}

pub(super) fn venue_media(venue: PlaceView) -> tl::enums::InputMedia {
    tl::types::InputMediaVenue {
        geo_point: input_geo_point(venue.point),
        title: venue.title,
        address: venue.address,
        provider: venue.provider,
        venue_id: venue.venue_id,
        venue_type: venue.venue_type,
    }
    .into()
}

fn input_geo_point(point: GeoPointView) -> tl::enums::InputGeoPoint {
    tl::types::InputGeoPoint {
        lat: f64::from(point.latitude_microdegrees) / 1_000_000.0,
        long: f64::from(point.longitude_microdegrees) / 1_000_000.0,
        accuracy_radius: None,
    }
    .into()
}

pub(super) fn normalize_place_results(results: Vec<tl::enums::BotInlineResult>) -> Vec<PlaceView> {
    results
        .into_iter()
        .filter_map(|result| match result.send_message() {
            tl::enums::BotInlineMessage::MediaVenue(venue) => normalize_venue(venue),
            _ => None,
        })
        .take(50)
        .collect()
}

pub(super) fn resolved_venue_bot(
    resolved_id: i64,
    users: &[tl::enums::User],
) -> Option<tl::enums::InputUser> {
    users.iter().find_map(|user| match user {
        tl::enums::User::User(user) if user.id == resolved_id && user.bot => {
            user.access_hash.map(|access_hash| {
                tl::types::InputUser {
                    user_id: user.id,
                    access_hash,
                }
                .into()
            })
        }
        _ => None,
    })
}

fn normalize_venue(venue: tl::types::BotInlineMessageMediaVenue) -> Option<PlaceView> {
    let tl::enums::GeoPoint::Point(point) = venue.geo else {
        return None;
    };
    let point = normalize_point(point.lat, point.long)?;
    Some(PlaceView {
        point,
        title: venue.title,
        address: venue.address,
        provider: venue.provider,
        venue_id: venue.venue_id,
        venue_type: venue.venue_type,
    })
}

fn normalize_point(latitude: f64, longitude: f64) -> Option<GeoPointView> {
    if !latitude.is_finite()
        || !longitude.is_finite()
        || !(-90.0..=90.0).contains(&latitude)
        || !(-180.0..=180.0).contains(&longitude)
    {
        return None;
    }
    Some(GeoPointView {
        latitude_microdegrees: (latitude * 1_000_000.0).round() as i32,
        longitude_microdegrees: (longitude * 1_000_000.0).round() as i32,
    })
}
