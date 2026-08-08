#[test]
fn delayed_message_results_update_their_destination_chat_only() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(hierarchy_bootstrap())),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::MoveDown)));

    let delayed = app.transition(Input::Adapter(AdapterEvent::MessageAdded {
        chat: ChatId(10),
        message: Box::new(MessageView {
            id: MessageId(4),
            sender: "You".to_owned(),
            body: "sent before switching".to_owned(),
            timestamp: "12:22".to_owned(),
            direction: MessageDirection::Outgoing,
            delivery: DeliveryState::Sent,
            reply_to: None,
            details: super::MessageDetails::default(),
        }),
    }));
    assert!(delayed.view.messages.is_empty());

    let first = app.transition(Input::Intent(Intent::Action(Action::MoveUp)));
    assert_eq!(
        first.view.messages.last().map(|message| message.id),
        Some(MessageId(4))
    );
}

#[test]
fn returning_to_the_composer_preserves_the_older_transcript_anchor() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::TargetPreviousMessage)),
    );
    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::TargetPreviousMessage)),
    );

    let composer = app.transition(Input::Intent(Intent::Action(Action::Cancel)));
    assert_eq!(composer.view.focus, Focus::Composer);
    assert_eq!(composer.view.active_message, None);
    assert_eq!(composer.view.transcript_anchor, Some(1));
}

#[test]
fn leaving_message_selection_resets_the_transcript_anchor() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::TargetPreviousMessage)),
    );
    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::TargetPreviousMessage)),
    );
    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::ToggleMessageSelection)),
    );

    let composer = app.transition(Input::Intent(Intent::Action(Action::Cancel)));

    assert_eq!(composer.view.focus, Focus::Composer);
    assert_eq!(composer.view.active_message, None);
    assert!(composer.view.selected_messages.is_empty());
    assert_eq!(composer.view.transcript_anchor, None);
}

#[test]
fn escape_ascends_the_hierarchy_and_folders_change_only_from_chat_list() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(hierarchy_bootstrap())),
    );
    let chat_list = app.transition(Input::Intent(Intent::Insert("does not enter".to_owned())));
    assert_eq!(chat_list.view.focus, Focus::Chats);
    assert!(chat_list.view.composer.text.is_empty());
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    let composer = app.transition(Input::Intent(Intent::Action(Action::NextFolder)));
    assert_eq!(composer.view.active_folder, 0);
    let targeted = app.transition(Input::Intent(Intent::Action(Action::TargetPreviousMessage)));
    assert_eq!(targeted.view.focus, Focus::Transcript);
    let newest = app.transition(Input::Intent(Intent::Action(Action::TargetNextMessage)));
    assert_eq!(newest.view.focus, Focus::Composer);
    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::TargetPreviousMessage)),
    );
    let composer = app.transition(Input::Intent(Intent::Action(Action::Cancel)));
    assert_eq!(composer.view.focus, Focus::Composer);
    let chats = app.transition(Input::Intent(Intent::Action(Action::Cancel)));
    assert_eq!(chats.view.focus, Focus::Chats);
    let folder = app.transition(Input::Intent(Intent::Action(Action::NextFolder)));
    assert_eq!(folder.view.active_folder, 1);
}

#[test]
fn reconnect_is_available_only_during_cooldown() {
    let mut app = App::new();
    assert!(!app.view().actions.contains(&Action::Reconnect));
    let cooldown = app.transition(Input::Adapter(AdapterEvent::ConnectionChanged(
        ConnectionState::ReconnectCooldown,
    )));
    assert!(cooldown.view.actions.contains(&Action::Reconnect));
    let reconnecting = app.transition(Input::Intent(Intent::Action(Action::Reconnect)));
    assert_eq!(reconnecting.view.connection, ConnectionState::Connecting);
    assert!(!reconnecting.view.actions.contains(&Action::Reconnect));
    assert_eq!(reconnecting.effect, Some(Effect::Reconnect));
}

#[test]
fn restored_connection_preserves_pending_messages_and_interaction_state() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(hierarchy_bootstrap())),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::MoveDown)));
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    apply(
        &mut app,
        Input::Intent(Intent::Insert("queued while offline".to_owned())),
    );
    let pending = app.transition(Input::Intent(Intent::Action(Action::Send)));
    let pending_id = pending
        .view
        .messages
        .last()
        .expect("optimistic Message should be visible")
        .id;

    let mut restored = hierarchy_bootstrap();
    restored.connection = ConnectionState::Connected;
    let update = app.transition(Input::Adapter(AdapterEvent::ConnectionRestored(restored)));

    assert_eq!(update.view.connection, ConnectionState::Connected);
    assert_eq!(update.view.focus, Focus::Composer);
    assert_eq!(update.view.active_chat, Some(1));
    assert!(
        update.view.messages.iter().any(|message| {
            message.id == pending_id && message.delivery == DeliveryState::Pending
        })
    );
}

#[test]
fn restored_connection_preserves_the_in_progress_composer() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(hierarchy_bootstrap())),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    apply(
        &mut app,
        Input::Intent(Intent::Insert("still typing".to_owned())),
    );

    let update = app.transition(Input::Adapter(AdapterEvent::ConnectionRestored(
        hierarchy_bootstrap(),
    )));

    assert_eq!(update.view.focus, Focus::Composer);
    assert_eq!(update.view.composer.text, "still typing");
}

#[test]
fn folder_picker_adds_and_removes_the_active_chat() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(hierarchy_bootstrap())),
    );

    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::ManageFolders)),
    );
    let adding = app.transition(Input::Intent(Intent::Action(
        Action::ToggleFolderMembership,
    )));
    assert_eq!(
        adding.effect,
        Some(Effect::SetChatFolder {
            chat: ChatId(10),
            folder: 1,
            included: true,
        })
    );
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::FolderMembershipChanged {
            chat: ChatId(10),
            folder: 1,
            included: true,
        }),
    );
    let work = app.transition(Input::Intent(Intent::Action(Action::NextFolder)));
    assert_eq!(
        work.view
            .chats
            .iter()
            .map(|chat| chat.id)
            .collect::<Vec<_>>(),
        vec![ChatId(10), ChatId(20)]
    );

    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::ManageFolders)),
    );
    let removing = app.transition(Input::Intent(Intent::Action(
        Action::ToggleFolderMembership,
    )));
    assert_eq!(
        removing.effect,
        Some(Effect::SetChatFolder {
            chat: ChatId(10),
            folder: 1,
            included: false,
        })
    );
}

#[test]
fn folder_mutation_without_inline_bootstrap_requests_fresh_reconciliation() {
    let mut fixture = hierarchy_bootstrap();
    fixture.folder_details.push(crate::FolderDetailsView {
        id: crate::FolderId(1),
        rules: Some(crate::FolderRulesView::default()),
        shareable: true,
    });
    fixture.chats[0].folders = vec![0];
    let mut app = App::new();
    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));

    let completed = app.transition(Input::Adapter(AdapterEvent::FolderOperationCompleted {
        result: crate::FolderOperationResult::Updated {
            id: crate::FolderId(1),
            title: "Focused".to_owned(),
            rules: Some(crate::FolderRulesView::default()),
        },
        reconciliation: None,
    }));
    assert_eq!(completed.effect, Some(Effect::RefreshFolders));

    let mut fresh_chat = hierarchy_bootstrap().chats[0].clone();
    fresh_chat.folders.push(1);
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::FolderReconciled(Box::new(
            crate::FolderReconciliation {
                folders: vec![
                    hierarchy_bootstrap().folders[0].clone(),
                    crate::FolderView {
                        id: 1,
                        title: "Focused".to_owned(),
                        unread: 0,
                    },
                ],
                details: vec![crate::FolderDetailsView {
                    id: crate::FolderId(1),
                    rules: Some(crate::FolderRulesView::default()),
                    shareable: true,
                }],
                chats: vec![fresh_chat],
            },
        ))),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::NextFolder)));
    assert!(app.view().chats.iter().any(|chat| chat.id == ChatId(10)));
}

#[test]
fn live_reconciliation_applies_edits_deletions_reads_and_archive_moves() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );
    let mut edited = app.view().messages[1].clone();
    edited.body = "edited body".to_owned();
    edited.details.edited = true;
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::MessageUpdated {
            chat: ChatId(10),
            message: Box::new(edited),
        }),
    );
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::MessageAdded {
            chat: ChatId(10),
            message: Box::new(MessageView {
                id: MessageId(4),
                sender: "You".to_owned(),
                body: "outgoing".to_owned(),
                timestamp: "12:01".to_owned(),
                direction: MessageDirection::Outgoing,
                delivery: DeliveryState::Sent,
                reply_to: None,
                details: super::MessageDetails::default(),
            }),
        }),
    );
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::HistoryRead {
            chat: ChatId(10),
            max_id: MessageId(4),
            outgoing: true,
            unread: None,
        }),
    );
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::MessagesDeleted {
            chat: None,
            ids: vec![MessageId(1)],
        }),
    );
    let reconciled = app.view();
    assert!(
        !reconciled
            .messages
            .iter()
            .any(|message| message.id == MessageId(1))
    );
    assert!(
        reconciled
            .messages
            .iter()
            .any(|message| message.id == MessageId(2)
                && message.body == "edited body"
                && message.details.edited)
    );
    assert!(
        reconciled.messages.iter().any(|message| {
            message.id == MessageId(4) && message.delivery == DeliveryState::Read
        })
    );

    let update = app.transition(Input::Adapter(AdapterEvent::ChatArchiveChanged {
        chat: ChatId(10),
        archived: true,
    }));
    assert!(update.view.chats.is_empty());
    assert_eq!(update.view.active_chat, None);
    assert!(update.view.messages.is_empty());
}

#[test]
fn unread_boundary_is_stable_across_history_and_live_updates_until_read() {
    let mut app = App::new();
    let fixture = bootstrap();
    let recent = fixture.messages.clone();
    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));

    assert_eq!(app.view().unread_boundary, Some(MessageId(2)));

    let mut refreshed = vec![MessageView {
        id: MessageId(0),
        sender: "Lin".to_owned(),
        body: "older".to_owned(),
        timestamp: "11:59".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails::default(),
    }];
    refreshed.extend(recent);
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::ChatLoaded {
            chat: ChatId(10),
            messages: refreshed,
            pinned_messages: Vec::new(),
        }),
    );
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::MessageAdded {
            chat: ChatId(10),
            message: Box::new(MessageView {
                id: MessageId(4),
                sender: "Lin".to_owned(),
                body: "live".to_owned(),
                timestamp: "12:01".to_owned(),
                direction: MessageDirection::Incoming,
                delivery: DeliveryState::Read,
                reply_to: None,
                details: MessageDetails::default(),
            }),
        }),
    );

    assert_eq!(app.view().unread_boundary, Some(MessageId(2)));

    apply(
        &mut app,
        Input::Adapter(AdapterEvent::HistoryRead {
            chat: ChatId(10),
            max_id: MessageId(3),
            outgoing: false,
            unread: Some(1),
        }),
    );
    assert_eq!(app.view().unread_boundary, Some(MessageId(4)));

    apply(
        &mut app,
        Input::Adapter(AdapterEvent::HistoryRead {
            chat: ChatId(10),
            max_id: MessageId(4),
            outgoing: false,
            unread: Some(0),
        }),
    );
    assert_eq!(app.view().unread_boundary, None);
}
use super::*;
