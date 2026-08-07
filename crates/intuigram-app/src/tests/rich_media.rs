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
