use super::{apply, bootstrap};
use crate::{
    Action, AdapterEvent, ChatId, DownloadId, DownloadView, Effect, Input, Intent, MediaCard,
    MediaKind, MessageId, TextEntity, TextEntityKind,
};

#[test]
fn image_preview_space_is_reserved_only_while_loading() {
    let mut fixture = bootstrap();
    fixture.messages[2].details.media = Some(MediaCard {
        kind: MediaKind::Photo,
        title: "Photo".to_owned(),
        description: "image".to_owned(),
        details: Vec::new(),
        poll: None,
        remote_id: Some("42".to_owned()),
    });
    let mut app = crate::App::new();

    let loading = app.transition(Input::Adapter(AdapterEvent::Bootstrap(fixture)));

    assert_eq!(
        loading.effect,
        Some(Effect::LoadMediaPreview {
            chat: ChatId(10),
            message: MessageId(3),
        })
    );
    assert_eq!(loading.view.media_preview_loads.len(), 1);
    assert!(loading.view.has_pending_effort());

    let failed = app.transition(Input::Adapter(AdapterEvent::MediaPreviewFailed {
        chat: ChatId(10),
        message: MessageId(3),
    }));
    assert!(failed.view.media_preview_loads.is_empty());
}

#[test]
fn safe_and_internal_links_route_without_leaving_the_state_owner() {
    let mut fixture = bootstrap();
    fixture.messages[2].body = "https://example.com".to_owned();
    fixture.messages[2].details.entities = vec![TextEntity {
        offset: 0,
        length: 19,
        kind: TextEntityKind::Url,
    }];
    let mut app = crate::App::new();
    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    for action in [Action::Open, Action::JumpLatest] {
        apply(&mut app, Input::Intent(Intent::Action(action)));
    }

    let external = app.transition(Input::Intent(Intent::Action(Action::OpenLink)));
    assert_eq!(
        external.effect,
        Some(Effect::OpenExternalLink {
            url: "https://example.com".to_owned(),
        })
    );

    let mut fixture = bootstrap();
    fixture.messages[2].body = "Intuigram".to_owned();
    fixture.messages[2].details.entities = vec![TextEntity {
        offset: 0,
        length: 9,
        kind: TextEntityKind::TextUrl {
            url: "https://t.me/intuigram".to_owned(),
        },
    }];
    let mut app = crate::App::new();
    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    for action in [Action::Open, Action::JumpLatest] {
        apply(&mut app, Input::Intent(Intent::Action(action)));
    }
    let internal = app.transition(Input::Intent(Intent::Action(Action::OpenLink)));
    assert_eq!(
        internal.effect,
        Some(Effect::ResolveTelegramUsername {
            username: "intuigram".to_owned(),
        })
    );
}

#[test]
fn disguised_links_show_the_exact_destination_before_opening() {
    let mut fixture = bootstrap();
    fixture.messages[2].body = "https://example.com".to_owned();
    fixture.messages[2].details.entities = vec![TextEntity {
        offset: 0,
        length: 19,
        kind: TextEntityKind::TextUrl {
            url: "https://evil.example/login".to_owned(),
        },
    }];
    let mut app = crate::App::new();
    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    for action in [Action::Open, Action::JumpLatest] {
        apply(&mut app, Input::Intent(Intent::Action(action)));
    }

    let warned = app.transition(Input::Intent(Intent::Action(Action::OpenLink)));
    assert_eq!(warned.effect, None);
    assert_eq!(
        warned
            .view
            .link_confirmation
            .as_ref()
            .map(|link| link.url.as_str()),
        Some("https://evil.example/login")
    );

    let confirmed = app.transition(Input::Intent(Intent::Action(Action::ConfirmOpenLink)));
    assert_eq!(
        confirmed.effect,
        Some(Effect::OpenExternalLink {
            url: "https://evil.example/login".to_owned(),
        })
    );
}

#[test]
fn downloaded_media_is_opened_or_revealed_by_opaque_handle() {
    let mut fixture = bootstrap();
    fixture.messages[2].details.media = Some(MediaCard {
        kind: MediaKind::File,
        title: "installer.sh".to_owned(),
        description: "application/x-shellscript".to_owned(),
        details: Vec::new(),
        poll: None,
        remote_id: Some("42".to_owned()),
    });
    let mut app = crate::App::new();
    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    for action in [Action::Open, Action::JumpLatest] {
        apply(&mut app, Input::Intent(Intent::Action(action)));
    }

    let requested = app.transition(Input::Intent(Intent::Action(Action::DownloadMedia)));
    assert_eq!(
        requested.effect,
        Some(Effect::DownloadMedia {
            chat: crate::ChatId(10),
            message: MessageId(3),
        })
    );

    apply(
        &mut app,
        Input::Adapter(AdapterEvent::DownloadReady {
            chat: crate::ChatId(10),
            download: DownloadView {
                chat: crate::ChatId(10),
                id: DownloadId(7),
                path: "/downloads/installer.sh".to_owned(),
                reveal_only: true,
                message: MessageId(3),
                preview: None,
            },
        }),
    );
    let opened = app.transition(Input::Intent(Intent::Action(Action::OpenDownload)));
    assert_eq!(
        opened.effect,
        Some(Effect::OpenDownload {
            download: DownloadId(7),
            reveal: true,
        })
    );
}
