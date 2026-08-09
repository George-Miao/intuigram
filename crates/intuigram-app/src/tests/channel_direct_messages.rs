use super::*;
use crate::{SavedDialogDraftView, SavedDialogListView, SavedDialogView};

fn direct_messages_bootstrap() -> Bootstrap {
    let mut fixture = bootstrap();
    fixture.chats[0] = ChatView {
        id: ChatId(-100),
        title: "Broadcast inbox".to_owned(),
        preview: String::new(),
        preview_sender: None,
        preview_sender_peer: None,
        preview_timestamp: String::new(),
        status: "12 subscribers".to_owned(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        has_topics: false,
        has_direct_messages: true,
        kind: ChatKind::Channel,
        folders: vec![0],
    };
    fixture.messages.clear();
    fixture.saved_dialog_lists = vec![SavedDialogListView {
        chat: ChatId(-100),
        dialogs: vec![dialog(ChatId(20), 2, Some("answer later"))],
    }];
    fixture
}

fn dialog(peer: ChatId, unread: u32, draft: Option<&str>) -> SavedDialogView {
    SavedDialogView {
        peer,
        title: "Ada".to_owned(),
        preview: "question".to_owned(),
        timestamp: "12:00".to_owned(),
        unread,
        unread_mark: false,
        pinned: false,
        top_message: MessageId(7),
        draft: draft.map(|text| SavedDialogDraftView {
            text: text.to_owned(),
            reply_to: None,
        }),
    }
}

fn direct_message(id: i64, thread_root: Option<MessageId>) -> MessageView {
    MessageView {
        id: MessageId(id),
        sender: "Ada".to_owned(),
        body: format!("direct {id}"),
        timestamp: "12:01".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Sent,
        reply_to: None,
        details: MessageDetails {
            thread_root,
            saved_peer: Some(ChatId(20)),
            ..MessageDetails::default()
        },
    }
}

#[test]
fn channel_direct_messages_reuse_dialog_navigation_and_restore_peer_draft() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(direct_messages_bootstrap())),
    );

    let list = app.transition(Input::Intent(Intent::Action(Action::Open)));
    assert_eq!(list.view.focus, Focus::SavedDialogs);
    assert_eq!(list.effect, Some(Effect::LoadSavedDialogs(ChatId(-100))));

    let opened = app.transition(Input::Intent(Intent::Action(Action::Open)));
    assert_eq!(opened.view.focus, Focus::Composer);
    assert_eq!(opened.view.active_saved_peer, Some(ChatId(20)));
    assert_eq!(opened.view.composer.text, "answer later");
    assert_eq!(
        opened.effect,
        Some(Effect::LoadSavedHistory {
            chat: ChatId(-100),
            peer: ChatId(20),
        })
    );
}

#[test]
fn direct_dialog_send_and_nested_thread_keep_the_peer_scope() {
    let mut app = App::new();
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::Bootstrap(direct_messages_bootstrap())),
    );
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    apply(
        &mut app,
        Input::Adapter(AdapterEvent::SavedHistoryLoaded {
            chat: ChatId(-100),
            peer: ChatId(20),
            messages: vec![direct_message(7, None)],
        }),
    );
    apply(&mut app, Input::Intent(Intent::Insert("reply".to_owned())));

    let sent = app.transition(Input::Intent(Intent::Action(Action::Send)));
    assert!(matches!(
        sent.effect,
        Some(Effect::SendMessage {
            chat: ChatId(-100),
            saved_peer: Some(ChatId(20)),
            thread_root: None,
            ..
        })
    ));

    apply(
        &mut app,
        Input::Intent(Intent::Action(Action::TargetPreviousMessage)),
    );
    let thread = app.transition(Input::Intent(Intent::Action(Action::OpenThread)));
    assert!(
        matches!(
            thread.effect,
            Some(Effect::LoadThread {
                chat: ChatId(-100),
                root: MessageId(_),
                saved_peer: Some(ChatId(20)),
            })
        ),
        "unexpected thread effect: {:?}",
        thread.effect
    );
}

#[test]
fn direct_dialog_read_updates_only_that_peer() {
    let mut fixture = direct_messages_bootstrap();
    fixture.saved_dialog_lists[0]
        .dialogs
        .push(dialog(ChatId(30), 4, None));
    let mut app = App::new();
    apply(&mut app, Input::Adapter(AdapterEvent::Bootstrap(fixture)));
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));
    apply(&mut app, Input::Intent(Intent::Action(Action::Open)));

    let loaded = app.transition(Input::Adapter(AdapterEvent::SavedHistoryLoaded {
        chat: ChatId(-100),
        peer: ChatId(20),
        messages: vec![direct_message(7, None)],
    }));
    assert_eq!(
        loaded.effect,
        Some(Effect::ReadHistory {
            chat: ChatId(-100),
            max_id: MessageId(7),
            saved_peer: Some(ChatId(20)),
        })
    );

    apply(
        &mut app,
        Input::Adapter(AdapterEvent::HistoryRead {
            chat: ChatId(-100),
            saved_peer: Some(ChatId(20)),
            max_id: MessageId(7),
            outgoing: false,
            unread: Some(0),
        }),
    );
    assert_eq!(app.view().saved_dialogs[0].unread, 0);
    assert_eq!(app.view().saved_dialogs[1].unread, 4);
}
