use grammers_tl_types::Serializable as _;

use super::refresh::*;
use super::*;

#[test]
fn refresh_requests_keep_the_exact_peer_and_content_identity() {
    let peer = tl::enums::InputPeer::User(tl::types::InputPeerUser {
        user_id: 77,
        access_hash: 700,
    });
    let paid = paid_refresh_request(peer.clone(), 81);
    assert_eq!(paid.peer, peer);
    assert_eq!(paid.id, [81]);
    let story = story_refresh_request(paid.peer.clone(), 12);
    assert_eq!(story.peer, paid.peer);
    assert_eq!(story.id, [12]);
    let giveaway = giveaway_refresh_request(story.peer.clone(), 83);
    assert_eq!(giveaway.peer, story.peer);
    assert_eq!(giveaway.msg_id, 83);
}

#[test]
fn paid_refresh_result_replaces_ordered_preview_state() {
    let updates = paid_update();
    assert_eq!(
        paid_items_from_updates(&updates, ChatId(10), MessageId(81)),
        Some(vec![PaidMediaItemView::Available {
            kind: MediaKind::Photo,
            title: "Photo".to_owned(),
            remote_id: None,
        }])
    );
    assert!(paid_items_from_updates(&updates, ChatId(10), MessageId(82)).is_none());
}

#[test]
fn live_paid_update_emits_a_typed_ordered_child_replacement() {
    let normalized = normalize_live_update(&paid_update().to_bytes(), &mut HashMap::new())
        .expect("the live paid update should normalize");

    assert!(matches!(
        normalized.events.as_slice(),
        [AdapterEvent::PaidMediaItemsUpdated {
            chat: ChatId(10),
            message: MessageId(81),
            items,
        }] if matches!(items.as_slice(), [PaidMediaItemView::Available { kind: MediaKind::Photo, .. }])
    ));
}

#[test]
fn story_refresh_result_keeps_requested_peer_and_story_identity() {
    let response = tl::types::stories::Stories {
        count: 1,
        stories: vec![
            tl::types::StoryItemSkipped {
                close_friends: true,
                live: false,
                id: 12,
                date: 1_786_310_400,
                expire_date: 1_786_396_800,
            }
            .into(),
        ],
        pinned_to_top: None,
        chats: Vec::new(),
        users: Vec::new(),
    }
    .into();
    let card = story_card_from_response(ChatId(77), 12, &response)
        .expect("the requested Story should normalize");
    assert!(matches!(
        card.specialized,
        Some(SpecializedMediaView::Story(SharedStoryView {
            peer: ChatId(77),
            id: 12,
            state: StoryStateView::Skipped,
            ..
        }))
    ));
}

#[test]
fn giveaway_refresh_result_preserves_participation_and_result() {
    let mut giveaway = giveaway_fixture();
    apply_giveaway_info(
        &mut giveaway,
        tl::types::payments::GiveawayInfo {
            participating: true,
            preparing_results: true,
            start_date: 1_786_310_400,
            joined_too_early_date: None,
            admin_disallowed_chat_id: None,
            disallowed_country: None,
        }
        .into(),
    );
    assert!(matches!(
        giveaway.info,
        Some(GiveawayInfoView::Active {
            participating: true,
            preparing_results: true,
            ..
        })
    ));
    apply_giveaway_info(
        &mut giveaway,
        tl::types::payments::GiveawayInfoResults {
            winner: true,
            refunded: false,
            start_date: 1_786_310_400,
            gift_code_slug: Some("prize-code".to_owned()),
            stars_prize: Some(1_000),
            finish_date: 1_786_396_800,
            winners_count: 3,
            activated_count: Some(2),
        }
        .into(),
    );
    assert_eq!(giveaway.stars, Some(1_000));
    assert_eq!(giveaway.winners_count, Some(3));
    assert!(matches!(
        giveaway.info,
        Some(GiveawayInfoView::Results {
            winner: true,
            activated_count: Some(2),
            ..
        })
    ));
}

fn giveaway_fixture() -> GiveawayView {
    GiveawayView {
        state: GiveawayStateView::Active,
        quantity: 3,
        premium_months: None,
        stars: None,
        prize_description: None,
        until_date: "2026-08-31".to_owned(),
        only_new_subscribers: false,
        winners_visible: true,
        country_codes: Vec::new(),
        channel_count: 1,
        winners_count: None,
        unclaimed_count: None,
        refunded: false,
        info: None,
    }
}

fn paid_update() -> tl::enums::Updates {
    tl::enums::Updates::UpdateShort(tl::types::UpdateShort {
        update: tl::types::UpdateMessageExtendedMedia {
            peer: tl::types::PeerUser { user_id: 10 }.into(),
            msg_id: 81,
            extended_media: vec![
                tl::types::MessageExtendedMedia {
                    media: tl::enums::MessageMedia::Photo(tl::types::MessageMediaPhoto {
                        spoiler: false,
                        live_photo: false,
                        photo: Some(tl::types::PhotoEmpty { id: 900 }.into()),
                        ttl_seconds: None,
                        video: None,
                    }),
                }
                .into(),
            ],
        }
        .into(),
        date: 1_786_310_400,
    })
}
