use super::bootstrap;
use crate::{
    AdapterEvent, App, ChatId, Effect, Input, MediaCard, MediaKind, MessageId, OfflineMediaTarget,
};

#[test]
fn retained_chat_media_is_fetched_serially_before_derived_previews() {
    let mut fixture = bootstrap();
    fixture.offline_chats = vec![ChatId(10)];
    fixture.messages[2].details.media = Some(MediaCard {
        kind: MediaKind::Photo,
        title: "Photo".to_owned(),
        description: String::new(),
        details: Vec::new(),
        poll: None,
        specialized: None,
        remote_id: Some("42".to_owned()),
    });
    let mut app = App::new();
    let target = OfflineMediaTarget {
        chat: ChatId(10),
        message: MessageId(3),
    };

    let started = app.transition(Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    assert_eq!(
        started.effect,
        Some(Effect::CacheMediaOffline {
            target,
            locator: None,
        })
    );
    assert_eq!(started.view.offline_chats, vec![ChatId(10)]);

    let retained = app.transition(Input::Adapter(AdapterEvent::MediaCachedOffline(target)));
    assert_eq!(
        retained.effect,
        Some(Effect::LoadMediaPreview {
            chat: ChatId(10),
            message: MessageId(3),
            locator: None,
        })
    );
}
