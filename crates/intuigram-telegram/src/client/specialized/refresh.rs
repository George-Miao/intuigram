use super::*;

impl Client {
    /// Refreshes one specialized Message family without initiating a purchase.
    pub async fn refresh_specialized(
        &mut self,
        chat: ChatId,
        message: MessageId,
        target: SpecializedRefreshTarget,
    ) -> Result<MediaCard> {
        match target {
            SpecializedRefreshTarget::PaidMedia => self.refresh_paid_media(chat, message).await,
            SpecializedRefreshTarget::Story { peer, id } => self.refresh_story(peer, id).await,
            SpecializedRefreshTarget::Giveaway => self.refresh_giveaway(chat, message).await,
        }
    }

    async fn refresh_paid_media(&mut self, chat: ChatId, message: MessageId) -> Result<MediaCard> {
        let original = self.message_media(chat, message).await?;
        let tl::enums::MessageMedia::PaidMedia(_) = original else {
            return SpecializedMediaUnavailableSnafu {
                message_id: message.0,
                family: "paid media",
            }
            .fail();
        };
        let peer = self.peers.resolve(chat)?;
        let id = telegram_message_id(message)?;
        let response = self
            .connection
            .invoke(&paid_refresh_request(peer, id))
            .await
            .context(InvokeSnafu)?;
        let Some(items) = paid_items_from_updates(&response, chat, message) else {
            return self
                .refreshed_family(chat, message, "paid media", |media| {
                    matches!(media, tl::enums::MessageMedia::PaidMedia(_))
                })
                .await;
        };
        let mut card = normalize_media(&original);
        let Some(SpecializedMediaView::PaidMedia(media)) = card.specialized.as_mut() else {
            return SpecializedMediaUnavailableSnafu {
                message_id: message.0,
                family: "paid media",
            }
            .fail();
        };
        media.items = items;
        Ok(card)
    }

    async fn refresh_story(&mut self, peer: ChatId, id: i32) -> Result<MediaCard> {
        let input_peer = self.peers.resolve(peer)?;
        let response = self
            .connection
            .invoke(&story_refresh_request(input_peer, id))
            .await
            .context(InvokeSnafu)?;
        let tl::enums::stories::Stories::Stories(stories) = &response;
        self.update_peer_cache(&stories.chats, &stories.users);
        story_card_from_response(peer, id, &response)
    }

    async fn refresh_giveaway(&mut self, chat: ChatId, message: MessageId) -> Result<MediaCard> {
        let peer = self.peers.resolve(chat)?;
        let msg_id = telegram_message_id(message)?;
        let info = self
            .connection
            .invoke(&giveaway_refresh_request(peer, msg_id))
            .await
            .context(InvokeSnafu)?;
        let mut card = self
            .refreshed_family(chat, message, "giveaway", |media| {
                matches!(
                    media,
                    tl::enums::MessageMedia::Giveaway(_)
                        | tl::enums::MessageMedia::GiveawayResults(_)
                )
            })
            .await?;
        let Some(SpecializedMediaView::Giveaway(giveaway)) = card.specialized.as_mut() else {
            return SpecializedMediaUnavailableSnafu {
                message_id: message.0,
                family: "giveaway",
            }
            .fail();
        };
        apply_giveaway_info(giveaway, info);
        Ok(card)
    }
}

pub(super) fn paid_refresh_request(
    peer: tl::enums::InputPeer,
    id: i32,
) -> tl::functions::messages::GetExtendedMedia {
    tl::functions::messages::GetExtendedMedia { peer, id: vec![id] }
}

pub(super) fn story_refresh_request(
    peer: tl::enums::InputPeer,
    id: i32,
) -> tl::functions::stories::GetStoriesById {
    tl::functions::stories::GetStoriesById { peer, id: vec![id] }
}

pub(super) fn giveaway_refresh_request(
    peer: tl::enums::InputPeer,
    msg_id: i32,
) -> tl::functions::payments::GetGiveawayInfo {
    tl::functions::payments::GetGiveawayInfo { peer, msg_id }
}

pub(super) fn story_card_from_response(
    peer: ChatId,
    id: i32,
    response: &tl::enums::stories::Stories,
) -> Result<MediaCard> {
    let tl::enums::stories::Stories::Stories(stories) = response;
    let story = stories
        .stories
        .iter()
        .find(|story| story.id() == id)
        .context(StoryUnavailableSnafu {
            peer_id: peer.0,
            story_id: id,
        })?;
    Ok(normalize_story_item(peer, id, Some(story), false))
}

pub(super) fn paid_items_from_updates(
    updates: &tl::enums::Updates,
    chat: ChatId,
    message: MessageId,
) -> Option<Vec<PaidMediaItemView>> {
    let find = |updates: &[tl::enums::Update]| {
        updates.iter().find_map(|update| match update {
            tl::enums::Update::MessageExtendedMedia(update)
                if marked_peer_id(&update.peer) == chat
                    && i64::from(update.msg_id) == message.0 =>
            {
                Some(normalize_paid_media_items(&update.extended_media))
            }
            _ => None,
        })
    };
    match updates {
        tl::enums::Updates::UpdateShort(update) => find(std::slice::from_ref(&update.update)),
        tl::enums::Updates::Combined(updates) => find(&updates.updates),
        tl::enums::Updates::Updates(updates) => find(&updates.updates),
        _ => None,
    }
}

pub(super) fn apply_giveaway_info(
    giveaway: &mut GiveawayView,
    info: tl::enums::payments::GiveawayInfo,
) {
    match info {
        tl::enums::payments::GiveawayInfo::Info(info) => {
            let issue = info
                .joined_too_early_date
                .map(|date| format!("joined too early on {}", format_date(date)))
                .or_else(|| {
                    info.admin_disallowed_chat_id
                        .map(|chat| format!("administrator of disallowed Chat {chat}"))
                })
                .or_else(|| {
                    info.disallowed_country
                        .map(|country| format!("country {country}"))
                });
            giveaway.info = Some(GiveawayInfoView::Active {
                participating: info.participating,
                preparing_results: info.preparing_results,
                start_date: format_date(info.start_date),
                eligibility_issue: issue,
            });
        }
        tl::enums::payments::GiveawayInfo::Results(info) => {
            giveaway.refunded = info.refunded;
            giveaway.winners_count = nonnegative_u32(Some(info.winners_count));
            giveaway.stars = info
                .stars_prize
                .and_then(|stars| u64::try_from(stars).ok())
                .or(giveaway.stars);
            giveaway.info = Some(GiveawayInfoView::Results {
                winner: info.winner,
                start_date: format_date(info.start_date),
                finish_date: format_date(info.finish_date),
                activated_count: nonnegative_u32(info.activated_count),
                gift_code_slug: info.gift_code_slug,
            });
        }
    }
}
