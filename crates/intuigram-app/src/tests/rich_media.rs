use super::*;
use crate::{RichMediaItemId, RichMediaItemView, RichMediaLibraryKind};

#[test]
fn library_loading_and_send_failure_remain_typed_application_state() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::OpenRichMedia)),
    );

    let loading = app.transition(Input::Intent(Intent::Action(Action::ChooseRichMedia)));
    assert_eq!(
        loading.effect,
        Some(Effect::BrowseRichMedia {
            kind: RichMediaLibraryKind::Stickers,
        })
    );
    assert_eq!(loading.view.actions, vec![Action::Quit]);

    apply(
        &mut app,
        Input::Adapter(AdapterEvent::RichMediaLibraryReady {
            kind: RichMediaLibraryKind::Stickers,
            items: vec![RichMediaItemView {
                id: RichMediaItemId(7),
                label: "wave".to_owned(),
            }],
        }),
    );
    let send = app.transition(Input::Intent(Intent::Action(Action::ChooseRichMedia)));
    assert!(matches!(send.effect, Some(Effect::SendLibraryMedia { .. })));
    assert_eq!(
        send.view.messages.last().map(|message| message.delivery),
        Some(DeliveryState::Pending)
    );

    apply(
        &mut app,
        Input::Adapter(AdapterEvent::RichMediaFailed {
            chat: ChatId(10),
            local_id: MessageId(-1),
            reason: "upload rejected".to_owned(),
        }),
    );
    assert_eq!(
        app.view().messages.last().map(|message| message.delivery),
        Some(DeliveryState::Failed)
    );
    assert_eq!(app.view().notice.as_deref(), Some("upload rejected"));
}

#[test]
fn acknowledged_rich_media_is_replaced_by_its_normalized_server_message() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::OpenRichMedia)),
    );
    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::ChooseRichMedia)),
    );
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::RichMediaLibraryReady {
            kind: RichMediaLibraryKind::Stickers,
            items: vec![RichMediaItemView {
                id: RichMediaItemId(7),
                label: "wave".to_owned(),
            }],
        }),
    );
    let sent = app.transition(Input::Intent(Intent::Action(Action::ChooseRichMedia)));
    let local_id = sent
        .view
        .messages
        .last()
        .expect("optimistic rich media should be visible")
        .id;
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::RichMediaAcknowledged {
            chat: ChatId(10),
            local_id,
        }),
    );

    apply(
        &mut app,
        Input::Adapter(AdapterEvent::MessageAdded {
            chat: ChatId(10),
            message: Box::new(MessageView {
                id: MessageId(77),
                sender: "You".to_owned(),
                body: "[Sticker] animated.webp".to_owned(),
                timestamp: "12:10".to_owned(),
                direction: MessageDirection::Outgoing,
                delivery: DeliveryState::Sent,
                reply_to: None,
                details: MessageDetails::default(),
            }),
        }),
    );

    assert!(!app.view().messages.iter().any(|message| message.id.0 < 0));
    assert_eq!(
        app.view()
            .messages
            .iter()
            .filter(|message| message.id == MessageId(77))
            .count(),
        1
    );
}
