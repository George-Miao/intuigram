use super::{apply, bootstrap};
use crate::{
    Action, AdapterEvent, ChatId, DownloadId, DownloadView, Effect, InlineImage, Input, Intent,
    MediaCard, MediaKind, MediaPreviewView, MessageId, TextEntity, TextEntityKind,
};

#[test]
fn displayed_media_previews_are_not_evicted_by_unrelated_preview_completions() {
    let mut app = crate::App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );
    let image = InlineImage::from_rgba(1, 1, vec![255, 0, 0, 255])
        .expect("fixture pixels should match their dimensions");

    for message in 1..=65 {
        apply(
            &mut app,
            Input::Adapter(AdapterEvent::MediaPreviewReady(MediaPreviewView {
                chat: ChatId(10),
                message: MessageId(message),
                image: image.clone(),
            })),
        );
    }

    let view = app.view();
    assert_eq!(view.media_previews.len(), 65);
    assert!(
        view.media_previews
            .iter()
            .any(|preview| preview.message == MessageId(1))
    );
}

#[test]
fn image_preview_space_is_reserved_only_while_loading() {
    let mut fixture = bootstrap();
    fixture.messages[2].details.media = Some(MediaCard {
        kind: MediaKind::Photo,
        title: "Photo".to_owned(),
        description: "image".to_owned(),
        details: Vec::new(),
        poll: None,
        specialized: None,
        remote_id: Some("42".to_owned()),
    });
    let mut app = crate::App::new();

    let loading = app.transition(Input::Adapter(AdapterEvent::Bootstrap(fixture)));

    assert_eq!(
        loading.effect,
        Some(Effect::LoadMediaPreview {
            chat: ChatId(10),
            message: MessageId(3),
            locator: None,
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
        specialized: None,
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
            destination: None,
            locator: None,
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

#[test]
fn save_as_collects_an_exact_destination_before_downloading() {
    let mut fixture = bootstrap();
    fixture.messages[2].details.media = Some(MediaCard {
        kind: MediaKind::File,
        title: "report.txt".to_owned(),
        description: "text/plain".to_owned(),
        details: Vec::new(),
        poll: None,
        specialized: None,
        remote_id: Some("42".to_owned()),
    });
    let mut app = crate::App::new();
    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    for action in [Action::Open, Action::JumpLatest, Action::SaveAs] {
        apply(&mut app, Input::Intent(Intent::Action(action)));
    }
    for _ in 0.."report.txt".len() {
        apply(&mut app, Input::Intent(Intent::Backspace));
    }
    apply(
        &mut app,
        Input::Intent(Intent::Insert("/chosen/report.txt".to_owned())),
    );

    let saved = app.transition(Input::Intent(Intent::Action(Action::ConfirmSaveAs)));

    assert_eq!(saved.view.save_as, None);
    assert_eq!(
        saved.effect,
        Some(Effect::DownloadMedia {
            chat: ChatId(10),
            message: MessageId(3),
            destination: Some("/chosen/report.txt".to_owned()),
            locator: None,
        })
    );
}
