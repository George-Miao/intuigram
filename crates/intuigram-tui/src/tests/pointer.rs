use super::*;

#[test]
fn primary_clicks_resolve_from_the_matching_rendered_semantics() {
    let current = pointer_view();
    let frame = render_test_frame(&current, 120, 40);
    let targets = [
        (SemanticRole::Folder, ActivationTarget::Folder(3)),
        (SemanticRole::Chat, ActivationTarget::Chat(ChatId(7))),
        (
            SemanticRole::Message,
            ActivationTarget::Message(MessageId(11)),
        ),
        (SemanticRole::Composer, ActivationTarget::Composer),
    ];

    for (role, target) in targets {
        let node = frame
            .semantics
            .iter()
            .find(|node| node.role == role)
            .expect("rendered target should have semantic bounds");
        let event = Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: node.bounds.x,
            row: node.bounds.y,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(
            resolve_test_frame_event(&current, &frame, event),
            Some(UiEvent::Intent(intuigram_lib::Intent::Activate(target)))
        );
    }
}

#[test]
fn modified_clicks_remain_available_to_the_terminal() {
    let current = view(Vec::new());
    let frame = render_test_frame(&current, 120, 40);

    assert_eq!(
        resolve_test_frame_event(
            &current,
            &frame,
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 0,
                row: 0,
                modifiers: KeyModifiers::SHIFT,
            }),
        ),
        None
    );
}

#[test]
fn composer_clicks_resolve_to_the_clicked_text_position() {
    let mut current = pointer_view();
    current.focus = Focus::Composer;
    current.composer.text = "first\nsecond".to_owned();
    current.composer.cursor = current.composer.text.len();
    let frame = render_test_frame(&current, 120, 40);
    let composer = frame
        .semantics
        .iter()
        .find(|node| node.role == SemanticRole::Composer)
        .expect("Composer should have semantic bounds");

    let event = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: composer.bounds.x + 3 + 3,
        row: composer.bounds.y + 2,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(
        resolve_test_frame_event(&current, &frame, event),
        Some(UiEvent::Intent(intuigram_lib::Intent::SetComposerCursor(9)))
    );
}

#[test]
fn action_bar_clicks_invoke_the_rendered_effective_action() {
    let mut current = pointer_view();
    current.focus = Focus::Composer;
    current.actions = vec![Action::Send, Action::Cancel];
    let frame = render_test_frame(&current, 120, 40);
    let action = frame
        .semantics
        .iter()
        .find(|node| node.role == SemanticRole::Action && node.bounds.width > 0)
        .expect("Action Bar should expose a clickable action");
    let expected = action
        .action
        .expect("Action semantic should retain its intent");

    let event = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: action.bounds.x,
        row: action.bounds.y,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(
        resolve_test_frame_event(&current, &frame, event),
        Some(UiEvent::Intent(intuigram_lib::Intent::Action(expected)))
    );
}

#[test]
fn folder_unread_count_is_part_of_its_pointer_target() {
    let mut current = view(Vec::new());
    current.folders = vec![
        FolderView {
            id: 3,
            title: "Work".to_owned(),
            unread: 123,
        },
        FolderView {
            id: 4,
            title: "Personal".to_owned(),
            unread: 0,
        },
    ];
    let frame = render_test_frame(&current, 120, 40);
    let work = frame
        .semantics
        .iter()
        .find(|node| node.role == SemanticRole::Folder && node.domain_id == Some(3))
        .expect("the Work Folder should have semantic bounds");
    let last_count_column = work.bounds.x + 7;

    assert_eq!(
        frame.buffer[(last_count_column, work.bounds.y + 1)].symbol(),
        "3"
    );
    assert_eq!(
        resolve_test_frame_event(
            &current,
            &frame,
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: last_count_column,
                row: work.bounds.y + 1,
                modifiers: KeyModifiers::NONE,
            }),
        ),
        Some(UiEvent::Intent(intuigram_lib::Intent::Activate(
            ActivationTarget::Folder(3)
        )))
    );
}

#[test]
fn mouse_wheel_emits_pane_aware_scroll_intents() {
    let current = pointer_view();
    let frame = render_test_frame(&current, 120, 40);

    for (role, kind, target, direction) in [
        (
            SemanticRole::Chat,
            MouseEventKind::ScrollDown,
            ScrollTarget::Chats,
            ScrollDirection::Down,
        ),
        (
            SemanticRole::Message,
            MouseEventKind::ScrollUp,
            ScrollTarget::Transcript,
            ScrollDirection::Up,
        ),
    ] {
        let node = frame
            .semantics
            .iter()
            .find(|node| node.role == role)
            .expect("the scroll region should have semantic bounds");
        assert_eq!(
            resolve_test_frame_event(
                &current,
                &frame,
                Event::Mouse(MouseEvent {
                    kind,
                    column: node.bounds.x,
                    row: node.bounds.y,
                    modifiers: KeyModifiers::NONE,
                }),
            ),
            Some(UiEvent::Intent(intuigram_lib::Intent::Scroll(
                target, direction
            )))
        );
    }
}

fn pointer_view() -> View {
    let mut current = view(Vec::new());
    current.folders.push(FolderView {
        id: 3,
        title: "Work".to_owned(),
        unread: 0,
    });
    current.chats.push(ChatView {
        id: ChatId(7),
        title: "Ada".to_owned(),
        preview: "hello".to_owned(),
        preview_sender: None,
        preview_sender_peer: None,
        preview_timestamp: String::new(),
        status: "online".to_owned(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        has_topics: false,
        has_direct_messages: false,
        kind: ChatKind::Private,
        folders: vec![3],
    });
    current.active_chat = Some(0);
    current.messages.push(MessageView {
        id: MessageId(11),
        sender: "Ada".to_owned(),
        body: "hello".to_owned(),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails::default(),
    });
    current
}
