use intuigram_lib::{SavedDialogDraftView, SavedDialogView};

use super::*;

#[test]
fn channel_direct_message_list_uses_direct_labels_unread_and_drafts() {
    let mut current = view(Vec::new());
    current.chats = vec![ChatView {
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
    }];
    current.active_chat = Some(0);
    current.focus = Focus::SavedDialogs;
    current.active_saved_dialog = Some(0);
    current.saved_dialogs = vec![SavedDialogView {
        peer: ChatId(20),
        title: "Ada".to_owned(),
        preview: "question".to_owned(),
        timestamp: "12:00".to_owned(),
        unread: 3,
        unread_mark: false,
        pinned: false,
        top_message: MessageId(7),
        draft: Some(SavedDialogDraftView {
            text: "answer later".to_owned(),
            reply_to: None,
        }),
    }];

    let frame = render_test_frame(&current, 100, 28);
    let rendered = frame
        .buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    assert!(rendered.contains("1 direct conversations"));
    assert!(rendered.contains("Ada"));
    assert!(rendered.contains("Draft · answer later"));
    assert!(rendered.contains("12:00 3"));
    assert!(frame.semantics.iter().any(|node| {
        node.role == SemanticRole::SavedDialogList && node.name == "Direct Messages"
    }));
}

#[test]
fn channel_direct_message_dialog_keeps_the_composer_visible() {
    let mut current = view(Vec::new());
    current.chats = vec![ChatView {
        id: ChatId(-100),
        title: "Broadcast inbox".to_owned(),
        preview: String::new(),
        preview_sender: None,
        preview_sender_peer: None,
        preview_timestamp: String::new(),
        status: String::new(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        has_topics: false,
        has_direct_messages: true,
        kind: ChatKind::Channel,
        folders: vec![0],
    }];
    current.active_chat = Some(0);
    current.active_saved_peer = Some(ChatId(20));
    current.focus = Focus::Composer;

    let frame = render_test_frame(&current, 100, 28);

    assert!(
        frame
            .semantics
            .iter()
            .any(|node| { node.role == SemanticRole::Composer && node.focused })
    );
}
