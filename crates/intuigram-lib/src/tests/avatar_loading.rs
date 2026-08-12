use super::*;
use crate::{AvatarId, AvatarRef, AvatarView, InlineImage};

#[test]
fn only_known_visible_avatar_peers_are_loaded_and_retained() {
    let mut fixture = bootstrap();
    fixture.avatar_peers = vec![avatar(10, 1), avatar(99, 2)];
    let mut app = App::new();

    let loading = app.transition(Input::Adapter(AdapterEvent::Bootstrap(fixture)));

    assert_eq!(
        loading.effect,
        Some(Effect::LoadAvatar {
            avatar: avatar(10, 1),
        })
    );
    assert_eq!(loading.view.avatar_loads, vec![avatar(10, 1)]);
    let image = InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
        .expect("fixture dimensions should match");
    let loaded = app.transition(Input::Adapter(AdapterEvent::AvatarReady(AvatarView {
        avatar: avatar(10, 1),
        image: image.clone(),
    })));

    assert_eq!(
        loaded.view.avatars,
        vec![AvatarView {
            avatar: avatar(10, 1),
            image
        }]
    );
    assert!(loaded.view.avatar_loads.is_empty());
    assert_eq!(loaded.effect, None);
}

#[test]
fn chat_list_previews_do_not_load_sender_avatars() {
    let mut fixture = bootstrap();
    fixture.chats[0].preview_sender = Some("Lin".to_owned());
    fixture.chats[0].preview_sender_peer = Some(ChatId(20));
    fixture.avatar_peers = vec![avatar(10, 1), avatar(20, 2)];
    let mut app = App::new();
    let loading = app.transition(Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    assert_eq!(
        loading.effect,
        Some(Effect::LoadAvatar {
            avatar: avatar(10, 1),
        })
    );
    let image = InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
        .expect("fixture dimensions should match");

    let completed = app.transition(Input::Adapter(AdapterEvent::AvatarReady(AvatarView {
        avatar: avatar(10, 1),
        image,
    })));

    assert_eq!(completed.effect, None);
}

#[test]
fn composition_configures_the_small_media_admission_capacity() {
    let mut fixture = bootstrap();
    for (id, title) in [(20, "Two"), (30, "Three")] {
        let mut chat = fixture.chats[0].clone();
        chat.id = ChatId(id);
        chat.title = title.to_owned();
        fixture.chats.push(chat);
    }
    fixture.avatar_peers = vec![avatar(10, 1), avatar(20, 2), avatar(30, 3)];
    let mut app = App::new();
    drop(app.transition(Input::ConfigureSmallMediaCapacity(2)));

    assert!(matches!(
        app.transition(Input::Adapter(AdapterEvent::Bootstrap(fixture)))
            .effect,
        Some(Effect::LoadAvatar { .. })
    ));
    assert!(matches!(
        app.transition(Input::EffectAccepted(crate::EffectAdmission::SmallMedia))
            .effect,
        Some(Effect::LoadAvatar { .. })
    ));
    assert_eq!(
        app.transition(Input::EffectAccepted(crate::EffectAdmission::SmallMedia))
            .effect,
        None
    );
}

#[test]
fn background_history_continues_after_visible_avatars_finish() {
    let mut fixture = hierarchy_bootstrap();
    fixture.avatar_peers = vec![avatar(10, 1)];
    let mut app = App::new();
    let loading = app.transition(Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    assert_eq!(
        loading.effect,
        Some(Effect::LoadAvatar {
            avatar: avatar(10, 1),
        })
    );
    let image = InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
        .expect("fixture dimensions should match");

    let completed = app.transition(Input::Adapter(AdapterEvent::AvatarReady(AvatarView {
        avatar: avatar(10, 1),
        image,
    })));

    assert!(matches!(
        completed.effect,
        Some(Effect::LoadChat {
            chat: ChatId(20),
            ..
        })
    ));
}

#[test]
fn a_new_avatar_revision_invalidates_loaded_pixels() {
    let mut fixture = bootstrap();
    fixture.avatar_peers = vec![avatar(10, 1)];
    let mut app = App::new();
    let _ = app.transition(Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    let image = InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
        .expect("fixture dimensions should match");
    let _ = app.transition(Input::Adapter(AdapterEvent::AvatarReady(AvatarView {
        avatar: avatar(10, 1),
        image,
    })));

    let mut refreshed = bootstrap();
    refreshed.avatar_peers = vec![avatar(10, 2)];
    let update = app.transition(Input::Adapter(AdapterEvent::ConnectionRestored(refreshed)));

    assert!(update.view.avatars.is_empty());
    assert!(matches!(update.effect, Some(Effect::LoadChat { .. })));
    let loaded = app.transition(Input::Adapter(AdapterEvent::ChatLoaded {
        chat: ChatId(10),
        status: None,
        messages: Vec::new(),
        pinned_messages: Vec::new(),
    }));
    assert_eq!(
        loaded.effect,
        Some(Effect::LoadAvatar {
            avatar: avatar(10, 2),
        })
    );
}

#[test]
fn a_live_avatar_change_queues_the_new_revision() {
    let mut fixture = bootstrap();
    fixture.avatar_peers = vec![avatar(10, 1)];
    let mut app = App::new();
    let _ = app.transition(Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    let image = InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
        .expect("fixture dimensions should match");
    let _ = app.transition(Input::Adapter(AdapterEvent::AvatarReady(AvatarView {
        avatar: avatar(10, 1),
        image,
    })));

    let update = app.transition(Input::Adapter(AdapterEvent::AvatarChanged {
        peer: ChatId(10),
        id: Some(AvatarId(2)),
    }));

    assert!(update.view.avatars.is_empty());
    assert_eq!(
        update.effect,
        Some(Effect::LoadAvatar {
            avatar: avatar(10, 2),
        })
    );
}

#[test]
fn a_stale_in_flight_avatar_is_discarded_before_loading_the_new_revision() {
    let mut fixture = bootstrap();
    fixture.avatar_peers = vec![avatar(10, 1)];
    let mut app = App::new();
    let loading = app.transition(Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    assert_eq!(
        loading.effect,
        Some(Effect::LoadAvatar {
            avatar: avatar(10, 1),
        })
    );

    let changed = app.transition(Input::Adapter(AdapterEvent::AvatarChanged {
        peer: ChatId(10),
        id: Some(AvatarId(2)),
    }));
    assert_eq!(
        changed.effect,
        Some(Effect::LoadAvatar {
            avatar: avatar(10, 2),
        })
    );
    let image = InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
        .expect("fixture dimensions should match");
    let completed = app.transition(Input::Adapter(AdapterEvent::AvatarReady(AvatarView {
        avatar: avatar(10, 1),
        image,
    })));

    assert!(completed.view.avatars.is_empty());
    assert_eq!(completed.effect, None);
}

const fn avatar(peer: i64, id: i64) -> AvatarRef {
    AvatarRef {
        peer: ChatId(peer),
        id: AvatarId(id),
    }
}
