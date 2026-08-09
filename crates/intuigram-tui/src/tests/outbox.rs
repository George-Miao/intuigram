use super::*;

#[test]
fn queued_outbox_message_replaces_generic_sending_metadata() {
    let current = outbox_view(OutboxStateView::Ready);

    let rendered = symbols(&render_test_frame(&current, 100, 28).buffer);

    assert!(rendered.contains("queued"));
    assert!(!rendered.contains("sending…"));
}

#[test]
fn transcript_presents_every_durable_lifecycle_and_retry_time() {
    let cases = [
        (OutboxStateView::Ready, "queued"),
        (OutboxStateView::InFlight, "sending…"),
        (OutboxStateView::Deferred, "retry 1970-01-01 00:00Z"),
        (OutboxStateView::CancelRequested, "cancelling…"),
        (OutboxStateView::Failed, "failed: network timeout"),
        (OutboxStateView::Conflict, "conflict"),
        (OutboxStateView::OutcomeUnknown, "outcome unknown"),
        (OutboxStateView::Expired, "expired"),
        (OutboxStateView::Cancelled, "cancelled"),
    ];

    for (state, expected) in cases {
        let mut current = outbox_view(state);
        let item = &mut current.outbox[0];
        item.available_at = (state == OutboxStateView::Deferred).then_some(0);
        item.last_error = (state == OutboxStateView::Failed).then(|| "network timeout".to_owned());

        let rendered = symbols(&render_test_frame(&current, 100, 28).buffer);

        assert!(
            rendered.contains(expected),
            "{state:?} should render {expected:?}, got {rendered:?}"
        );
        assert!(!rendered.contains("failed !"));
    }
}

#[test]
fn narrow_transcript_uses_a_compact_but_useful_lifecycle_label() {
    let current = outbox_view(OutboxStateView::OutcomeUnknown);

    let rendered = symbols(&render_test_frame(&current, 24, 28).buffer);

    assert!(rendered.contains("unknown"));
    assert!(!rendered.contains("outcome unknown"));
}

#[test]
fn pending_outbox_work_animates_in_status_without_claiming_every_item_is_sending() {
    let mut current = outbox_view(OutboxStateView::Deferred);
    current.outbox[0].available_at = Some(0);
    current.outbox[0].local_message = None;
    current.messages.clear();

    let first = render_test_frame(&current, 100, 28);
    let first_status = row_text(&first.buffer, 26);
    let first_highlight = highlighted_columns(&first.buffer, 26);

    assert!(first_status.contains("outbox 1 pending"));
    assert!(!first_status.contains("sending"));

    current.animation_frame = 1;
    let second = render_test_frame(&current, 100, 28);
    let second_highlight = highlighted_columns(&second.buffer, 26);
    assert_ne!(first_highlight, second_highlight);
}

#[test]
fn message_action_labels_use_the_shared_menu_binding_without_direct_shortcuts() {
    let mut current = view(vec![
        Action::MoveUp,
        Action::MoveDown,
        Action::ChooseAction,
        Action::Cancel,
    ]);
    current.action_menu = Some(ActionMenuView {
        title: "Message Actions".to_owned(),
        selected: 1,
        items: vec![
            ActionMenuItemView {
                action: Action::RetryOutbox,
                label: "Retry Pending Operation".to_owned(),
            },
            ActionMenuItemView {
                action: Action::DismissOutbox,
                label: "Dismiss Pending Operation".to_owned(),
            },
        ],
    });
    let keymap = EffectiveKeymap::defaults();

    let rendered = symbols(&render_test_frame(&current, 100, 28).buffer);
    assert!(rendered.contains("Retry Pending Operation"));
    assert!(rendered.contains("Dismiss Pending Operation"));
    assert_eq!(
        keymap.resolve(&current, KeyChord::plain(Key::Enter)),
        Some(Action::ChooseAction)
    );

    current.action_menu = None;
    current.actions = vec![
        Action::CancelOutbox,
        Action::RetryOutbox,
        Action::ResolveOutbox,
        Action::DismissOutbox,
    ];
    assert_eq!(keymap.help(&current).count(), 0);
}

fn outbox_view(state: OutboxStateView) -> View {
    let mut current = view(Vec::new());
    current.chats = vec![ChatView {
        id: ChatId(10),
        title: "Intuigram".to_owned(),
        preview: String::new(),
        preview_sender: None,
        preview_sender_peer: None,
        preview_timestamp: String::new(),
        status: String::new(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        has_topics: false,
        has_direct_messages: false,
        kind: ChatKind::Private,
        folders: vec![0],
    }];
    current.active_chat = Some(0);
    current.focus = Focus::Transcript;
    current.messages = vec![MessageView {
        id: MessageId(-1),
        sender: "You".to_owned(),
        body: "durable message".to_owned(),
        timestamp: "now".to_owned(),
        direction: MessageDirection::Outgoing,
        delivery: match state {
            OutboxStateView::Ready
            | OutboxStateView::InFlight
            | OutboxStateView::Deferred
            | OutboxStateView::CancelRequested => DeliveryState::Pending,
            OutboxStateView::Failed
            | OutboxStateView::Conflict
            | OutboxStateView::OutcomeUnknown
            | OutboxStateView::Expired
            | OutboxStateView::Cancelled => DeliveryState::Failed,
        },
        reply_to: None,
        details: MessageDetails::default(),
    }];
    current.outbox = vec![OutboxItemView {
        key: OutboxKey(7),
        chat: ChatId(10),
        local_message: Some(MessageId(-1)),
        state,
        retryable: true,
        available_at: None,
        expires_at: None,
        last_error: None,
    }];
    current
}

fn symbols(buffer: &ratatui::buffer::Buffer) -> String {
    buffer.content.iter().map(|cell| cell.symbol()).collect()
}

fn row_text(buffer: &ratatui::buffer::Buffer, row: u16) -> String {
    (0..buffer.area.width)
        .map(|column| buffer[(column, row)].symbol())
        .collect()
}

fn highlighted_columns(buffer: &ratatui::buffer::Buffer, row: u16) -> Vec<u16> {
    (0..buffer.area.width)
        .filter(|column| buffer[(*column, row)].fg == Color::Rgb(141, 161, 1))
        .collect()
}
