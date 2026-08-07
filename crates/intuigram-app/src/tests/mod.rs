use super::{
    Action, ActivationTarget, AdapterEvent, App, Bootstrap, ChatId, ChatKind, ChatLoadingState,
    ChatView, ConnectionState, DeliveryState, Effect, Focus, FolderView, HistoryView, Input,
    Intent, MessageDetails, MessageDirection, MessageId, MessageView, SearchScope, SelectionView,
    TranscriptAnchorView,
};

fn bootstrap() -> Bootstrap {
    Bootstrap {
        connection: ConnectionState::Connected,
        account_name: "Ada".to_owned(),
        notification_identity: "telegram:10".to_owned(),
        restored_selection: None,
        transcript_anchors: Vec::new(),
        folders: vec![FolderView {
            id: 0,
            title: "All".to_owned(),
            unread: 2,
        }],
        chats: vec![ChatView {
            id: ChatId(10),
            title: "Intuigram".to_owned(),
            preview: "daily driver".to_owned(),
            status: String::new(),
            unread: 2,
            pinned: true,
            can_pin_messages: true,
            kind: ChatKind::Supergroup,
            folders: vec![0],
        }],
        messages: (1..=3)
            .map(|id| MessageView {
                id: MessageId(id),
                sender: "Lin".to_owned(),
                body: format!("message {id}"),
                timestamp: "12:00".to_owned(),
                direction: MessageDirection::Incoming,
                delivery: DeliveryState::Read,
                reply_to: None,
                details: super::MessageDetails::default(),
            })
            .collect(),
        pinned_messages: Vec::new(),
        drafts: Vec::new(),
        histories: Vec::new(),
    }
}

fn hierarchy_bootstrap() -> Bootstrap {
    let mut fixture = bootstrap();
    fixture.folders.push(FolderView {
        id: 1,
        title: "Work".to_owned(),
        unread: 0,
    });
    fixture.chats.push(ChatView {
        id: ChatId(20),
        title: "Rust".to_owned(),
        preview: "owned buffers".to_owned(),
        status: String::new(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        kind: ChatKind::Supergroup,
        folders: vec![0, 1],
    });
    fixture
}

fn apply(app: &mut App, input: Input) {
    drop(app.transition(input));
}
#[test]
fn reducer_applies_one_input_synchronously() {
    let mut app = App::new();

    let update = app.transition(Input::Adapter(AdapterEvent::Bootstrap(bootstrap())));

    assert_eq!(update.view.account_name, "Ada");
    assert_eq!(update.view.connection, ConnectionState::Connected);
    assert_eq!(update.effect, None);
}

#[test]
fn bootstrap_restores_persisted_root_and_thread_drafts() {
    let mut fixture = bootstrap();
    fixture.drafts = vec![
        super::DraftView {
            chat: ChatId(10),
            thread_root: None,
            text: "root draft".to_owned(),
            reply_to: Some(MessageId(2)),
        },
        super::DraftView {
            chat: ChatId(10),
            thread_root: Some(MessageId(3)),
            text: "thread draft".to_owned(),
            reply_to: None,
        },
    ];
    let mut app = App::new();

    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    assert_eq!(app.view().composer.text, "root draft");
    assert_eq!(app.view().composer.reply_to, Some(MessageId(2)));

    apply(&mut app, Input::Intent(Intent::Action(Action::JumpLatest)));
    apply(&mut app, Input::Intent(Intent::Action(Action::OpenThread)));
    assert_eq!(app.view().composer.text, "thread draft");
}

#[test]
fn switching_folder_rebuilds_the_chat_list_from_normalized_membership() {
    let mut fixture = hierarchy_bootstrap();
    fixture.chats[0].folders = vec![0];
    fixture.chats[1].folders = vec![0, 1];
    let mut app = App::new();
    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));

    apply(&mut app, Input::Intent(Intent::Action(Action::NextFolder)));

    let view = app.view();
    assert_eq!(view.active_folder, 1);
    assert_eq!(view.chats.len(), 1);
    assert_eq!(view.chats[0].id, ChatId(20));
    assert_eq!(view.active_chat, Some(0));
}

#[test]
fn bootstrap_restores_a_valid_folder_and_chat_selection() {
    let mut fixture = hierarchy_bootstrap();
    fixture.restored_selection = Some(SelectionView {
        folder: 1,
        chat: Some(ChatId(20)),
        message: None,
    });
    let mut app = App::new();

    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));

    assert_eq!(app.view().active_folder, 1);
    assert_eq!(app.view().chats.len(), 1);
    assert_eq!(app.view().chats[0].id, ChatId(20));
    assert_eq!(app.view().active_chat, Some(0));
}

#[test]
fn bootstrap_restores_the_per_account_transcript_anchor() {
    let mut fixture = bootstrap();
    fixture.restored_selection = Some(SelectionView {
        folder: 0,
        chat: Some(ChatId(10)),
        message: Some(MessageId(2)),
    });
    let mut app = App::new();

    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));

    assert_eq!(app.view().transcript_anchor, Some(1));
}

#[test]
fn bootstrap_restores_independent_transcript_anchors_when_switching_chats() {
    let mut fixture = hierarchy_bootstrap();
    fixture.histories.push(HistoryView {
        chat: ChatId(20),
        thread_root: None,
        messages: vec![MessageView {
            id: MessageId(20),
            sender: "Ferris".to_owned(),
            body: "older position".to_owned(),
            timestamp: "12:30".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Read,
            reply_to: None,
            details: MessageDetails::default(),
        }],
    });
    fixture.transcript_anchors = vec![
        TranscriptAnchorView {
            chat: ChatId(10),
            thread: None,
            message: MessageId(2),
        },
        TranscriptAnchorView {
            chat: ChatId(20),
            thread: None,
            message: MessageId(20),
        },
    ];
    let mut app = App::new();
    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));

    apply(&mut app, Input::Intent(Intent::Action(Action::MoveDown)));

    assert_eq!(app.view().active_chat, Some(1));
    assert_eq!(app.view().transcript_anchor, Some(0));
}

#[test]
fn incoming_message_outside_the_visible_chat_uses_the_account_notification_identity() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );

    let update = app.transition(Input::Adapter(AdapterEvent::MessageAdded {
        chat: ChatId(10),
        message: Box::new(MessageView {
            id: MessageId(4),
            sender: "Lin".to_owned(),
            body: "notification body stays out of the effect".to_owned(),
            timestamp: "12:31".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Sent,
            reply_to: None,
            details: MessageDetails::default(),
        }),
    }));

    assert_eq!(
        update.effect,
        Some(Effect::Notify {
            identity: "telegram:10".to_owned(),
            chat: ChatId(10),
        })
    );
}

#[test]
fn bootstrap_clears_a_stale_selection_and_returns_to_the_default_folder() {
    let mut fixture = hierarchy_bootstrap();
    fixture.restored_selection = Some(SelectionView {
        folder: 1,
        chat: Some(ChatId(999)),
        message: None,
    });
    let mut app = App::new();

    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));

    assert_eq!(app.view().active_folder, 0);
    assert_eq!(app.view().active_chat, None);
    assert!(app.view().messages.is_empty());
}

#[test]
fn removing_the_active_chat_from_a_folder_rebinds_its_message_history() {
    let mut fixture = hierarchy_bootstrap();
    fixture.chats[0].folders.push(1);
    let mut app = App::new();
    let background = app.transition(Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    assert_eq!(
        background.effect,
        Some(Effect::LoadChat {
            chat: ChatId(20),
            selection: None,
            transcript_anchors: Vec::new(),
        })
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::NextFolder)));

    let update = app.transition(Input::Adapter(AdapterEvent::FolderMembershipChanged {
        chat: ChatId(10),
        folder: 1,
        included: false,
    }));

    assert_eq!(update.view.chats[0].id, ChatId(20));
    assert_eq!(update.view.active_chat, Some(0));
    assert!(update.view.messages.is_empty());
    assert_eq!(update.effect, None);
}

#[test]
fn new_messages_do_not_snap_transcript_while_reading_older_history() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    apply(&mut app, Input::Intent(Intent::Action(Action::JumpLatest)));
    let older = app.transition(Input::Intent(Intent::Action(Action::TargetPreviousMessage)));
    assert_eq!(older.view.active_message, Some(1));

    let updated = app.transition(Input::Adapter(AdapterEvent::MessageAdded {
        chat: ChatId(10),
        message: Box::new(MessageView {
            id: MessageId(4),
            sender: "Lin".to_owned(),
            body: "new".to_owned(),
            timestamp: "12:01".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Sent,
            reply_to: None,
            details: super::MessageDetails::default(),
        }),
    }));

    assert_eq!(updated.view.active_message, Some(1));
    assert!(updated.view.has_newer_messages);
}

#[test]
fn passive_message_updates_refresh_the_chat_list_without_a_history_reload() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );

    let updated = app.transition(Input::Adapter(AdapterEvent::MessageAdded {
        chat: ChatId(10),
        message: Box::new(MessageView {
            id: MessageId(4),
            sender: "Lin".to_owned(),
            body: "live update".to_owned(),
            timestamp: "12:02".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Sent,
            reply_to: None,
            details: super::MessageDetails::default(),
        }),
    }));

    assert_eq!(updated.view.chats[0].preview, "live update");
    assert_eq!(updated.view.chats[0].unread, 3);
    assert_eq!(
        updated.view.messages.last().map(|message| message.id),
        Some(MessageId(4))
    );
}

#[test]
fn search_scope_and_reply_send_follow_current_context() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(bootstrap())),
    );
    let search = app.transition(Input::Intent(Intent::Action(Action::Search)));
    assert_eq!(
        search.view.search.expect("search should be open").scope,
        SearchScope::Account
    );
    for action in [
        Action::Cancel,
        Action::Open,
        Action::TargetPreviousMessage,
        Action::Reply,
    ] {
        apply(&mut app, Input::Intent(Intent::Action(action)));
    }
    apply(&mut app, Input::Intent(Intent::Insert("hello".to_owned())));
    apply(&mut app, Input::Intent(Intent::Action(Action::Newline)));
    apply(&mut app, Input::Intent(Intent::Insert("world".to_owned())));
    let sent = app.transition(Input::Intent(Intent::Action(Action::Send)));
    assert_eq!(
        sent.effect,
        Some(Effect::SendMessage {
            chat: ChatId(10),
            text: "hello\nworld".to_owned(),
            entities: Vec::new(),
            link_preview: true,
            reply_to: Some(MessageId(3)),
            thread_root: None,
            attachments: Vec::new(),
            local_id: MessageId(-1),
        })
    );
    assert_eq!(
        sent.view.messages.last().map(|message| message.delivery),
        Some(DeliveryState::Pending)
    );
    assert_eq!(sent.view.focus, Focus::Composer);
}
mod click_activation;
mod clipboard;
mod history_loading;
mod link_media;
mod messaging;
mod reconciliation;
