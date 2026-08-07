use super::*;
use crate::{AttachmentId, AttachmentKind, AttachmentView};

#[test]
fn clipboard_text_and_media_candidates_join_the_active_composer() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    let attachments = vec![
        AttachmentView {
            id: AttachmentId(1),
            kind: AttachmentKind::Photo,
            name: "clipboard.png".to_owned(),
        },
        AttachmentView {
            id: AttachmentId(2),
            kind: AttachmentKind::Video,
            name: "clip.mp4".to_owned(),
        },
        AttachmentView {
            id: AttachmentId(3),
            kind: AttachmentKind::File,
            name: "notes.pdf".to_owned(),
        },
    ];

    let pasted = app.transition(Input::Adapter(AdapterEvent::ClipboardReady {
        chat: ChatId(10),
        thread_root: None,
        text: Some("caption".to_owned()),
        attachments: attachments.clone(),
    }));

    assert_eq!(pasted.view.composer.text, "caption");
    assert_eq!(pasted.view.composer.attachments, attachments);
    let sent = app.transition(Input::Intent(Intent::Action(Action::Send)));
    assert!(matches!(
        sent.effect,
        Some(Effect::SendMessage { attachments, .. })
            if attachments == vec![AttachmentId(1), AttachmentId(2), AttachmentId(3)]
    ));
}

#[test]
fn clipboard_failure_is_visible_without_changing_the_draft() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    apply(
        &mut app,
        Input::Intent(Intent::Insert("keep me".to_owned())),
    );

    let failed = app.transition(Input::Adapter(AdapterEvent::OperationFailed(
        "Clipboard has no supported content".to_owned(),
    )));

    assert_eq!(failed.view.composer.text, "keep me");
    assert_eq!(
        failed.view.notice.as_deref(),
        Some("Clipboard has no supported content")
    );
}
