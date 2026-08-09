use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use compio_mtproto::InvocationError;
use grammers_tl_types::{self as tl, Serializable as _};
use intuigram_app::{
    AdapterEvent, ChatId, ChatKind, ChatView, FolderId, GiftKindView, GiftView, GiveawayStateView,
    GiveawayView, MediaKind, MessageDirection, MessageId, SharedStoryView, SpecializedMediaView,
    StoryStateView, TodoListView,
};

use crate::UpdateScope;
use crate::source::{
    Error, LoginCodeDelivery, LoginCodeDeliveryMethod, LoginErrorAction, PeerDirectory,
    chat_traits, contains_login_token_update, direct_data_centers, ensure_production_environment,
    flood_wait_delay, login_error_action, normalize_code_delivery, normalize_code_delivery_method,
    normalize_dialog_folder_details, normalize_dialog_folders, normalize_live_update,
    normalize_serialized_media, normalize_serialized_peer_kind, qr_login_uri, rpc_migration_dc,
    service_event_description, service_event_media, set_dialog_filter_membership,
    thread_root_message_id,
};

#[test]
fn every_flood_wait_is_retried_after_the_server_delay() {
    let error = InvocationError::Rpc {
        code: 420,
        message: "FLOOD_WAIT_17".to_owned(),
    };

    assert_eq!(
        flood_wait_delay(&error),
        Some(std::time::Duration::from_secs(17))
    );
    assert_eq!(
        flood_wait_delay(&error),
        Some(std::time::Duration::from_secs(17))
    );
}

#[test]
fn invalid_peer_rpc_requests_one_directory_refresh() {
    let invalid = Error::Invoke {
        source: InvocationError::Rpc {
            code: 400,
            message: "CHANNEL_INVALID".to_owned(),
        },
    };
    let unrelated = Error::Invoke {
        source: InvocationError::Rpc {
            code: 400,
            message: "MESSAGE_ID_INVALID".to_owned(),
        },
    };

    assert!(invalid.requires_peer_refresh());
    assert!(!unrelated.requires_peer_refresh());
}

#[test]
fn qr_login_routes_session_password_needed_to_2fa() {
    let error = InvocationError::Rpc {
        code: 401,
        message: "SESSION_PASSWORD_NEEDED".to_owned(),
    };

    assert_eq!(
        login_error_action(&error),
        LoginErrorAction::RequestPassword
    );
}

#[test]
fn phone_login_retries_plain_and_diagnostic_auth_restarts() {
    for message in ["AUTH_RESTART", "AUTH_RESTART_7"] {
        let error = InvocationError::Rpc {
            code: 500,
            message: message.to_owned(),
        };

        assert_eq!(login_error_action(&error), LoginErrorAction::Restart);
    }
}

#[test]
fn test_data_center_configuration_is_rejected() {
    assert!(matches!(
        ensure_production_environment(true),
        Err(Error::TestDataCenter)
    ));
    assert!(ensure_production_environment(false).is_ok());
}

#[test]
fn qr_login_uri_uses_unpadded_url_safe_base64() {
    assert_eq!(qr_login_uri(&[0xfb, 0xff]), "tg://login?token=-_8");
}

#[test]
fn login_token_update_is_detected_inside_update_short() {
    let update = tl::enums::Updates::UpdateShort(tl::types::UpdateShort {
        update: tl::enums::Update::LoginToken,
        date: 1_700_000_000,
    });

    assert!(contains_login_token_update(&update.to_bytes()));
}

#[test]
fn unrelated_update_is_not_treated_as_a_login_scan() {
    assert!(!contains_login_token_update(
        &tl::enums::Updates::TooLong.to_bytes()
    ));
}

#[test]
fn updates_too_long_requests_reconciliation_without_a_fake_event() {
    let mut names = HashMap::new();

    let batch = normalize_live_update(&tl::enums::Updates::TooLong.to_bytes(), &mut names)
        .expect("known gap constructor should normalize");

    assert!(batch.events.is_empty());
    assert_eq!(batch.cursors.len(), 1);
    assert!(batch.cursors[0].gap);
}

#[test]
fn service_actions_have_clear_semantic_fallbacks() {
    assert_eq!(
        service_event_description(&tl::enums::MessageAction::HistoryClear),
        "Cleared Chat history"
    );
    assert_eq!(
        service_event_description(&tl::enums::MessageAction::ChatJoinedByRequest),
        "Joined after approval"
    );
    assert_eq!(
        service_event_description(&tl::enums::MessageAction::Empty),
        "Empty Telegram service event"
    );
}

#[test]
fn ordinary_replies_remain_in_root_history_while_threads_keep_their_root() {
    let ordinary = reply_header(Some(40), None, false);
    let first_topic_reply = reply_header(Some(40), None, true);
    let nested_thread_reply = reply_header(Some(41), Some(40), false);

    assert_eq!(thread_root_message_id(&ordinary), None);
    assert_eq!(
        thread_root_message_id(&first_topic_reply),
        Some(MessageId(40))
    );
    assert_eq!(
        thread_root_message_id(&nested_thread_reply),
        Some(MessageId(40))
    );
}

fn reply_header(
    reply_to: Option<i32>,
    top: Option<i32>,
    forum_topic: bool,
) -> tl::enums::MessageReplyHeader {
    tl::types::MessageReplyHeader {
        reply_to_scheduled: false,
        forum_topic,
        quote: false,
        reply_to_ephemeral: false,
        reply_to_msg_id: reply_to,
        reply_to_peer_id: None,
        reply_from: None,
        reply_media: None,
        reply_to_top_id: top,
        quote_text: None,
        quote_entities: None,
        quote_offset: None,
        todo_item_id: None,
        poll_option: None,
    }
    .into()
}

#[test]
fn passive_mutations_are_normalized_before_cursor_exposure() {
    let update = tl::enums::Updates::Updates(tl::types::Updates {
        updates: vec![
            tl::types::UpdateDeleteMessages {
                messages: vec![40, 41],
                pts: 10,
                pts_count: 2,
            }
            .into(),
            tl::types::UpdateReadHistoryOutbox {
                peer: tl::types::PeerUser { user_id: 7 }.into(),
                max_id: 42,
                pts: 11,
                pts_count: 1,
            }
            .into(),
            tl::types::UpdateFolderPeers {
                folder_peers: vec![
                    tl::types::FolderPeer {
                        peer: tl::types::PeerUser { user_id: 7 }.into(),
                        folder_id: 1,
                    }
                    .into(),
                ],
                pts: 12,
                pts_count: 1,
            }
            .into(),
        ],
        users: Vec::new(),
        chats: Vec::new(),
        date: 1_700_000_001,
        seq: 4,
    });
    let mut names = HashMap::new();

    let batch = normalize_live_update(&update.to_bytes(), &mut names)
        .expect("serialized mutation batch should normalize");

    assert_eq!(batch.cursors[0].pts, Some(12));
    assert_eq!(batch.cursors[0].seq, Some(4));
    assert!(matches!(
        &batch.events[0],
        AdapterEvent::MessagesDeleted { chat: None, ids }
            if ids == &vec![MessageId(40), MessageId(41)]
    ));
    assert!(matches!(
        &batch.events[1],
        AdapterEvent::HistoryRead {
            chat: ChatId(7),
            saved_peer: None,
            max_id: MessageId(42),
            outgoing: true,
            unread: None,
        }
    ));
    assert!(matches!(
        &batch.events[2],
        AdapterEvent::ChatArchiveChanged {
            chat: ChatId(7),
            archived: true,
        }
    ));
}

#[test]
fn pinned_message_deltas_are_normalized_with_their_chat_and_state() {
    let update = tl::enums::Updates::UpdateShort(tl::types::UpdateShort {
        update: tl::types::UpdatePinnedMessages {
            pinned: true,
            peer: tl::types::PeerUser { user_id: 7 }.into(),
            messages: vec![40, 42],
            pts: 9,
            pts_count: 1,
        }
        .into(),
        date: 1_700_000_000,
    });
    let mut names = HashMap::new();

    let batch = normalize_live_update(&update.to_bytes(), &mut names)
        .expect("serialized pin update should normalize");

    assert!(matches!(
        &batch.events[0],
        AdapterEvent::MessagesPinChanged {
            chat: ChatId(7),
            ids,
            pinned: true,
        } if ids == &vec![MessageId(40), MessageId(42)]
    ));

    let channel_update = tl::enums::Updates::UpdateShort(tl::types::UpdateShort {
        update: tl::types::UpdatePinnedChannelMessages {
            pinned: false,
            channel_id: 99,
            messages: vec![42],
            pts: 10,
            pts_count: 1,
        }
        .into(),
        date: 1_700_000_001,
    });
    let channel = normalize_live_update(&channel_update.to_bytes(), &mut names)
        .expect("serialized channel unpin update should normalize");
    let marked_channel = ChatId(-1_000_000_000_099);

    assert_eq!(
        channel.cursors[0].scope,
        UpdateScope::Channel(marked_channel)
    );
    assert_eq!(channel.cursors[0].pts, Some(10));
    assert!(matches!(
        &channel.events[0],
        AdapterEvent::MessagesPinChanged {
            chat,
            ids,
            pinned: false,
        } if *chat == marked_channel && ids == &vec![MessageId(42)]
    ));
}

#[test]
fn channel_pts_never_advance_the_account_cursor() {
    let update = tl::enums::Updates::Updates(tl::types::Updates {
        updates: vec![
            tl::types::UpdateFolderPeers {
                folder_peers: Vec::new(),
                pts: 12,
                pts_count: 1,
            }
            .into(),
            tl::types::UpdateDeleteChannelMessages {
                channel_id: 5,
                messages: vec![42],
                pts: 30,
                pts_count: 1,
            }
            .into(),
        ],
        users: Vec::new(),
        chats: Vec::new(),
        date: 1_700_000_001,
        seq: 4,
    });
    let mut names = HashMap::new();

    let batch = normalize_live_update(&update.to_bytes(), &mut names)
        .expect("mixed Account and Channel update should normalize");

    assert_eq!(batch.cursors.len(), 2);
    assert_eq!(batch.cursors[0].scope, UpdateScope::Account);
    assert_eq!(batch.cursors[0].pts, Some(12));
    assert_eq!(
        batch.cursors[1].scope,
        UpdateScope::Channel(ChatId(-1_000_000_000_005))
    );
    assert_eq!(batch.cursors[1].pts, Some(30));
}

#[test]
fn login_code_delivery_preserves_the_telegram_app_destination() {
    let delivery = normalize_code_delivery(tl::types::auth::SentCodeTypeApp { length: 5 }.into());

    assert_eq!(delivery, LoginCodeDelivery::TelegramApp { length: 5 });
}

#[test]
fn login_code_fallback_preserves_sms_delivery() {
    assert_eq!(
        normalize_code_delivery_method(&tl::enums::auth::CodeType::Sms),
        LoginCodeDeliveryMethod::Sms
    );
}

#[test]
fn phone_migration_rpc_error_exposes_its_target_data_center() {
    let error = InvocationError::Rpc {
        code: 303,
        message: "PHONE_MIGRATE_1".to_owned(),
    };

    assert_eq!(rpc_migration_dc(&error, "PHONE_MIGRATE_"), Some(1));
    assert_eq!(rpc_migration_dc(&error, "NETWORK_MIGRATE_"), None);
}

#[test]
fn direct_data_center_selection_ignores_incompatible_endpoints() {
    let direct = dc_option(1, "149.154.175.53", 443, false, false);
    let ipv6 = dc_option(1, "2001:db8::1", 443, true, false);
    let media = dc_option(2, "149.154.167.151", 443, false, true);

    let selected = direct_data_centers(vec![direct, ipv6, media]);

    assert_eq!(
        selected.get(&1),
        Some(&SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(149, 154, 175, 53)),
            443
        ))
    );
    assert!(!selected.contains_key(&2));
}
mod dialogs_and_peers;
mod live_updates;
mod media_fixtures;
mod notifications;
mod specialized_media;
mod topics;

use media_fixtures::{basic_group, channel, dc_option, user};
