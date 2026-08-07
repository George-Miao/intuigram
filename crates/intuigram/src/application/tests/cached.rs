use intuigram_app::{
    ChatId, ChatKind, DeliveryState, MediaCard, MediaKind, MessageDetails, MessageDirection,
    MessageId, MessageView, SelectionView, TextEntity, TextEntityKind,
};
use intuigram_store::{CachedAccount, StoredChat, StoredDraft, StoredFolder, StoredSelection};

use super::super::{cached_bootstrap, encode_stored_message};

#[test]
fn cached_account_restores_rich_thread_history_and_drafts() {
    let message = MessageView {
        id: MessageId(42),
        sender: "Ada".to_owned(),
        body: "cached caption".to_owned(),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: Some(MessageId(40)),
        details: MessageDetails {
            entities: vec![TextEntity {
                offset: 0,
                length: 6,
                kind: TextEntityKind::Bold,
            }],
            media: Some(MediaCard {
                kind: MediaKind::Photo,
                title: "Photo".to_owned(),
                description: "image".to_owned(),
                details: Vec::new(),
                poll: None,
                remote_id: Some("99".to_owned()),
            }),
            thread_root: Some(MessageId(41)),
            ..MessageDetails::default()
        },
    };
    let mut old_pin = message.clone();
    old_pin.id = MessageId(5);
    old_pin.details.thread_root = None;
    old_pin.details.pinned = true;
    let cached = CachedAccount {
        cursors: Vec::new(),
        folders: vec![StoredFolder {
            id: 0,
            title: "All".to_owned(),
            unread: 1,
        }],
        chats: vec![StoredChat {
            id: 7,
            kind: "private".to_owned(),
            title: "Ada".to_owned(),
            preview: "cached caption".to_owned(),
            status: "online".to_owned(),
            unread: 1,
            pinned: false,
            can_pin_messages: true,
            folders: vec![0],
        }],
        messages: vec![encode_stored_message(ChatId(7), &message)],
        pinned_messages: vec![encode_stored_message(ChatId(7), &old_pin)],
        drafts: vec![StoredDraft {
            chat_id: 7,
            thread_root: Some(41),
            text: "cached Draft".to_owned(),
            reply_to: Some(42),
            modified_at: 10,
        }],
        selection: Some(StoredSelection {
            folder_id: 0,
            chat_id: Some(7),
            anchor_message_id: Some(42),
            transcript_anchors: vec![intuigram_store::StoredTranscriptAnchor {
                chat_id: 7,
                thread_root: Some(41),
                message_id: 42,
            }],
        }),
    };

    let bootstrap = cached_bootstrap("Ada".to_owned(), "telegram:7".to_owned(), cached);

    assert_eq!(bootstrap.chats[0].kind, ChatKind::Private);
    assert_eq!(bootstrap.histories[0].thread_root, Some(MessageId(41)));
    assert_eq!(bootstrap.histories[0].messages, vec![message]);
    assert_eq!(bootstrap.pinned_messages[0].messages, vec![old_pin]);
    assert_eq!(bootstrap.drafts[0].text, "cached Draft");
    assert_eq!(
        bootstrap.restored_selection,
        Some(SelectionView {
            folder: 0,
            chat: Some(ChatId(7)),
            message: Some(MessageId(42)),
        })
    );
}
