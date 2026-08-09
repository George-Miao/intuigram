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
    assert_eq!(loaded.effect, None);
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
    assert_eq!(changed.effect, None);
    let image = InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
        .expect("fixture dimensions should match");
    let completed = app.transition(Input::Adapter(AdapterEvent::AvatarReady(AvatarView {
        avatar: avatar(10, 1),
        image,
    })));

    assert!(completed.view.avatars.is_empty());
    assert_eq!(
        completed.effect,
        Some(Effect::LoadAvatar {
            avatar: avatar(10, 2),
        })
    );
}

const fn avatar(peer: i64, id: i64) -> AvatarRef {
    AvatarRef {
        peer: ChatId(peer),
        id: AvatarId(id),
    }
}
