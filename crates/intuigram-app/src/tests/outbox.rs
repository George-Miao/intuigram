use super::*;
use crate::{OutboxAction, OutboxItemView, OutboxKey, OutboxStateView, View};

#[test]
fn bootstrap_projects_durable_outbox_state_onto_optimistic_messages() {
    let mut fixture = bootstrap();
    fixture.messages.push(outgoing(-1, DeliveryState::Saving));
    fixture.outbox = vec![outbox_item(OutboxStateView::Deferred, true)];
    let mut app = App::new();

    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));

    assert_eq!(app.view().outbox.len(), 1);
    assert_eq!(delivery(&app.view(), MessageId(-1)), DeliveryState::Pending);
}

#[test]
fn admission_and_terminal_events_update_delivery_without_removing_the_message() {
    let mut fixture = bootstrap();
    fixture.messages.push(outgoing(-1, DeliveryState::Saving));
    let mut app = App::new();
    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));

    apply(
        &mut app,
        Input::Adapter(AdapterEvent::OutboxChanged(outbox_item(
            OutboxStateView::Ready,
            true,
        ))),
    );
    assert_eq!(delivery(&app.view(), MessageId(-1)), DeliveryState::Pending);

    apply(
        &mut app,
        Input::Adapter(AdapterEvent::OutboxChanged(outbox_item(
            OutboxStateView::Failed,
            true,
        ))),
    );
    assert_eq!(delivery(&app.view(), MessageId(-1)), DeliveryState::Failed);

    apply(
        &mut app,
        Input::Adapter(AdapterEvent::OutboxRemoved { item: OutboxKey(7) }),
    );
    assert!(app.view().outbox.is_empty());
    assert_eq!(delivery(&app.view(), MessageId(-1)), DeliveryState::Failed);
}

#[test]
fn message_actions_offer_only_safe_outbox_resolutions() {
    let mut fixture = bootstrap();
    fixture.messages.push(outgoing(-1, DeliveryState::Failed));
    fixture.outbox = vec![outbox_item(OutboxStateView::Failed, true)];
    let mut app = App::new();
    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::TargetPreviousMessage)),
    );

    apply(&mut app, Input::Intent(Intent::Action(Action::OpenActions)));
    let view = app.view();
    let menu = view.action_menu.as_ref().expect("menu should open");
    assert!(
        menu.items
            .iter()
            .any(|item| item.action == Action::RetryOutbox)
    );
    assert!(
        menu.items
            .iter()
            .any(|item| item.action == Action::DismissOutbox)
    );
    assert!(
        !menu
            .items
            .iter()
            .any(|item| item.action == Action::CancelOutbox)
    );

    apply(&mut app, Input::Intent(Intent::Action(Action::Cancel)));
    let effect = app.transition(Input::Intent(Intent::Action(Action::RetryOutbox)));
    assert_eq!(
        effect.effect,
        Some(Effect::ResolveOutbox {
            item: OutboxKey(7),
            action: OutboxAction::Retry,
        })
    );
}

#[test]
fn unknown_outcomes_require_explicit_resolution_instead_of_retry() {
    let mut fixture = bootstrap();
    fixture.messages.push(outgoing(-1, DeliveryState::Failed));
    fixture.outbox = vec![outbox_item(OutboxStateView::OutcomeUnknown, false)];
    let mut app = App::new();
    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::TargetPreviousMessage)),
    );

    apply(&mut app, Input::Intent(Intent::Action(Action::OpenActions)));
    let actions = app
        .view()
        .action_menu
        .expect("menu should open")
        .items
        .into_iter()
        .map(|item| item.action)
        .collect::<Vec<_>>();
    assert!(actions.contains(&Action::ResolveOutbox));
    assert!(actions.contains(&Action::DismissOutbox));
    assert!(!actions.contains(&Action::RetryOutbox));
}

#[test]
fn active_outbox_states_keep_effort_animation_running_without_a_local_message() {
    for state in [
        OutboxStateView::Ready,
        OutboxStateView::InFlight,
        OutboxStateView::Deferred,
        OutboxStateView::CancelRequested,
    ] {
        let mut fixture = bootstrap();
        let mut item = outbox_item(state, true);
        item.local_message = None;
        fixture.outbox = vec![item];
        let mut app = App::new();

        apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));

        assert!(app.view().has_pending_effort(), "{state:?} should animate");
    }
}

#[test]
fn durable_edit_acknowledgement_preserves_unrelated_message_metadata() {
    let mut fixture = bootstrap();
    fixture.messages[1].reply_to = Some(MessageId(1));
    let mut app = App::new();
    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));

    apply(
        &mut app,
        Input::Adapter(AdapterEvent::MessageEditAcknowledged {
            chat: ChatId(10),
            message: MessageId(2),
            text: "edited".to_owned(),
            entities: Vec::new(),
        }),
    );

    let view = app.view();
    let message = view
        .messages
        .iter()
        .find(|message| message.id == MessageId(2))
        .expect("edited Message should remain present");
    assert_eq!(message.body, "edited");
    assert_eq!(message.reply_to, Some(MessageId(1)));
    assert!(message.details.edited);
}

#[test]
fn scheduled_acknowledgement_requests_a_fresh_server_projection() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );

    let update = app.transition(Input::Adapter(
        AdapterEvent::ScheduledOperationAcknowledged {
            chat: ChatId(10),
            saved_peer: None,
            notice: "Scheduled Message created".to_owned(),
        },
    ));

    assert_eq!(
        update.effect,
        Some(Effect::LoadScheduledMessages {
            chat: ChatId(10),
            saved_peer: None,
        })
    );
}

fn outgoing(id: i64, delivery: DeliveryState) -> MessageView {
    MessageView {
        id: MessageId(id),
        sender: "You".to_owned(),
        body: "durable".to_owned(),
        timestamp: "now".to_owned(),
        direction: MessageDirection::Outgoing,
        delivery,
        reply_to: None,
        details: MessageDetails::default(),
    }
}

fn outbox_item(state: OutboxStateView, retryable: bool) -> OutboxItemView {
    OutboxItemView {
        key: OutboxKey(7),
        chat: ChatId(10),
        local_message: Some(MessageId(-1)),
        state,
        retryable,
        available_at: None,
        expires_at: None,
        last_error: Some("network failed".to_owned()),
    }
}

fn delivery(view: &View, id: MessageId) -> DeliveryState {
    view.messages
        .iter()
        .find(|message| message.id == id)
        .expect("optimistic message should remain visible")
        .delivery
}
