use super::*;
use crate::{ScheduledDeliveryView, ScheduledMessageId, ScheduledMessageView, ScheduledRequest};

#[test]
fn scheduled_history_is_separate_and_adapter_failures_leave_it_visible() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));

    let open = app.transition(Input::Intent(Intent::Action(Action::OpenScheduled)));
    assert_eq!(
        open.effect,
        Some(Effect::LoadScheduledMessages {
            chat: ChatId(10),
            saved_peer: None,
        })
    );
    assert_eq!(open.view.messages.len(), 3);
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::ScheduledMessagesReady {
            chat: ChatId(10),
            saved_peer: None,
            messages: vec![ScheduledMessageView {
                id: ScheduledMessageId(70),
                delivery: ScheduledDeliveryView::WhenOnline,
                summary: "later".to_owned(),
            }],
        }),
    );
    assert_eq!(app.view().messages.len(), 3);

    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::NewScheduled)),
    );
    apply(&mut app, Input::Intent(Intent::Insert("new".to_owned())));
    apply(&mut app, Input::Intent(Intent::Action(Action::MoveDown)));
    apply(&mut app, Input::Intent(Intent::Insert("online".to_owned())));
    let save = app.transition(Input::Intent(Intent::Action(Action::SaveScheduled)));
    assert_eq!(
        save.effect,
        Some(Effect::ScheduledOperation {
            chat: ChatId(10),
            saved_peer: None,
            request: ScheduledRequest::Create {
                delivery: ScheduledDeliveryView::WhenOnline,
                text: "new".to_owned(),
            },
        })
    );

    apply(
        &mut app,
        Input::Adapter(AdapterEvent::ScheduledOperationFailed {
            chat: ChatId(10),
            saved_peer: None,
            reason: "schedule rejected".to_owned(),
        }),
    );
    let view = app.view();
    assert_eq!(view.messages.len(), 3);
    assert_eq!(
        view.scheduled.map(|manager| manager.messages.len()),
        Some(1)
    );
    assert_eq!(view.notice.as_deref(), Some("schedule rejected"));
}
