#[test]
fn quiz_options_results_and_solution_are_normalized() {
    let answers = [("Compio", vec![1]), ("Tokio", vec![2])]
        .into_iter()
        .map(|(text, option)| {
            tl::types::PollAnswer {
                text: text_entities(text),
                option,
                media: None,
                added_by: None,
                date: None,
            }
            .into()
        })
        .collect();
    let media = tl::enums::MessageMedia::Poll(Box::new(tl::types::MessageMediaPoll {
        poll: tl::types::Poll {
            id: 77,
            closed: false,
            public_voters: false,
            multiple_choice: false,
            quiz: true,
            open_answers: false,
            revoting_disabled: false,
            shuffle_answers: false,
            hide_results_until_close: false,
            creator: false,
            subscribers_only: false,
            question: text_entities("Which runtime?"),
            answers,
            close_period: None,
            close_date: None,
            countries_iso2: None,
            hash: 0,
        }
        .into(),
        results: tl::types::PollResults {
            min: false,
            has_unread_votes: false,
            can_view_stats: false,
            results: Some(vec![
                tl::types::PollAnswerVoters {
                    chosen: true,
                    correct: true,
                    option: vec![1],
                    voters: Some(3),
                    recent_voters: Some(Vec::new()),
                }
                .into(),
            ]),
            total_voters: Some(5),
            recent_voters: None,
            solution: Some("Completion-based I/O".to_owned()),
            solution_entities: Some(Vec::new()),
            solution_media: None,
        }
        .into(),
        attached_media: None,
    }));

    let card =
        normalize_serialized_media(&media.to_bytes()).expect("current-layer quiz should normalize");
    let poll = card.poll.expect("quiz should retain interactive state");

    assert_eq!(card.title, "Quiz");
    assert_eq!(card.description, "Which runtime?");
    assert_eq!(poll.total_voters, Some(5));
    assert_eq!(poll.options.len(), 2);
    assert!(poll.options[0].chosen);
    assert!(poll.options[0].correct);
    assert_eq!(poll.options[0].voters, Some(3));
    assert_eq!(poll.solution.as_deref(), Some("Completion-based I/O"));
}

#[test]
fn static_locations_preserve_exact_coordinates() {
    let location = tl::enums::MessageMedia::Geo(tl::types::MessageMediaGeo {
        geo: tl::types::GeoPoint {
            long: 139.6917,
            lat: 35.6895,
            access_hash: 1,
            accuracy_radius: Some(10),
        }
        .into(),
    });

    let card =
        normalize_serialized_media(&location.to_bytes()).expect("static location should normalize");

    assert_eq!(card.kind, MediaKind::Location);
    assert_eq!(card.description, "35.689500, 139.691700");
}

fn text_entities(text: &str) -> tl::enums::TextWithEntities {
    tl::types::TextWithEntities {
        text: text.to_owned(),
        entities: Vec::new(),
    }
    .into()
}

pub(crate) fn user(id: i64, is_self: bool, bot: bool) -> tl::types::User {
    tl::types::User {
        is_self,
        contact: false,
        mutual_contact: false,
        deleted: false,
        bot,
        bot_chat_history: false,
        bot_nochats: false,
        verified: false,
        restricted: false,
        min: false,
        bot_inline_geo: false,
        support: false,
        scam: false,
        apply_min_photo: false,
        fake: false,
        bot_attach_menu: false,
        premium: false,
        attach_menu_enabled: false,
        bot_can_edit: false,
        close_friend: false,
        stories_hidden: false,
        stories_unavailable: false,
        contact_require_premium: false,
        bot_business: false,
        bot_has_main_app: false,
        bot_forum_view: false,
        bot_forum_can_manage_topics: false,
        bot_can_manage_bots: false,
        bot_guestchat: false,
        bot_guard: false,
        id,
        access_hash: None,
        first_name: Some("Peer".to_owned()),
        last_name: None,
        username: None,
        phone: None,
        photo: None,
        status: None,
        bot_info_version: bot.then_some(1),
        restriction_reason: None,
        bot_inline_placeholder: None,
        lang_code: None,
        emoji_status: None,
        usernames: None,
        stories_max_id: None,
        color: None,
        profile_color: None,
        bot_active_users: None,
        bot_verification_icon: None,
        send_paid_messages_stars: None,
    }
}

pub(super) fn basic_group() -> tl::types::Chat {
    tl::types::Chat {
        creator: false,
        left: false,
        deactivated: false,
        call_active: false,
        call_not_empty: false,
        noforwards: false,
        id: 5,
        title: "Basic group".to_owned(),
        photo: tl::enums::ChatPhoto::Empty,
        participants_count: 2,
        date: 0,
        version: 1,
        migrated_to: None,
        admin_rights: None,
        default_banned_rights: None,
    }
}

pub(super) fn channel(broadcast: bool, gigagroup: bool) -> tl::types::Channel {
    tl::types::Channel {
        creator: false,
        left: false,
        broadcast,
        verified: false,
        megagroup: !broadcast,
        restricted: false,
        signatures: false,
        min: false,
        scam: false,
        has_link: false,
        has_geo: false,
        slowmode_enabled: false,
        call_active: false,
        call_not_empty: false,
        fake: false,
        gigagroup,
        noforwards: false,
        join_to_send: false,
        join_request: false,
        forum: false,
        stories_hidden: false,
        stories_hidden_min: false,
        stories_unavailable: false,
        signature_profiles: false,
        autotranslation: false,
        broadcast_messages_allowed: false,
        monoforum: false,
        forum_tabs: false,
        id: 6,
        access_hash: Some(7),
        title: "Channel".to_owned(),
        username: None,
        photo: tl::enums::ChatPhoto::Empty,
        date: 0,
        restriction_reason: None,
        admin_rights: None,
        banned_rights: None,
        default_banned_rights: None,
        participants_count: None,
        usernames: None,
        stories_max_id: None,
        color: None,
        profile_color: None,
        emoji_status: None,
        level: None,
        subscription_until_date: None,
        bot_verification_icon: None,
        send_paid_messages_stars: None,
        linked_monoforum_id: None,
    }
}

pub(super) fn dc_option(
    id: i32,
    ip_address: &str,
    port: i32,
    ipv6: bool,
    media_only: bool,
) -> tl::enums::DcOption {
    tl::types::DcOption {
        ipv6,
        media_only,
        tcpo_only: false,
        cdn: false,
        r#static: false,
        this_port_only: false,
        id,
        ip_address: ip_address.to_owned(),
        port,
        secret: None,
    }
    .into()
}
use super::*;
