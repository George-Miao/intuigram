use super::*;
use crate::source::NotificationDefaults;

#[test]
fn live_peer_mute_updates_normalize_into_chat_notification_state() {
    let update = tl::enums::Update::NotifySettings(tl::types::UpdateNotifySettings {
        peer: tl::types::NotifyPeer {
            peer: tl::types::PeerUser { user_id: 7 }.into(),
        }
        .into(),
        notify_settings: notification_settings(Some(i32::MAX)),
    });
    let mut names = HashMap::new();

    let batch = normalize_live_update(&update.to_bytes(), &mut names)
        .expect("serialized notification settings should normalize");

    assert_eq!(
        batch.events,
        vec![AdapterEvent::ChatMuteChanged {
            chat: ChatId(7),
            muted: true,
        }]
    );
}

#[test]
fn expired_or_inherited_notification_settings_are_not_muted() {
    assert!(crate::source::notifications_muted_at(
        &notification_settings(Some(101)),
        100,
        false,
    ));
    assert!(!crate::source::notifications_muted_at(
        &notification_settings(Some(100)),
        100,
        false,
    ));
    assert!(!crate::source::notifications_muted_at(
        &notification_settings(None),
        100,
        false,
    ));
}

#[test]
fn inherited_peer_settings_use_the_category_mute_default() {
    assert!(crate::source::notifications_muted_at(
        &notification_settings(None),
        100,
        true,
    ));
    assert!(!crate::source::notifications_muted_at(
        &notification_settings(Some(0)),
        100,
        true,
    ));
}

#[test]
fn notification_defaults_follow_telegram_chat_categories() {
    let defaults = NotificationDefaults::new(true, false, true);

    assert!(defaults.muted(ChatKind::Private));
    assert!(defaults.muted(ChatKind::Bot));
    assert!(!defaults.muted(ChatKind::Supergroup));
    assert!(defaults.muted(ChatKind::Channel));
}

fn notification_settings(mute_until: Option<i32>) -> tl::enums::PeerNotifySettings {
    tl::types::PeerNotifySettings {
        show_previews: None,
        silent: None,
        mute_until,
        ios_sound: None,
        android_sound: None,
        other_sound: None,
        stories_muted: None,
        stories_hide_sender: None,
        stories_ios_sound: None,
        stories_android_sound: None,
        stories_other_sound: None,
    }
    .into()
}
