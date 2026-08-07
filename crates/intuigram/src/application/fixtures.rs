#[cfg(test)]
pub(super) fn application_fixture() -> Bootstrap {
    Bootstrap {
        connection: intuigram_app::ConnectionState::Connected,
        account_name: "Intuigram Test".to_owned(),
        restored_selection: None,
        folders: vec![
            FolderView {
                id: 0,
                title: "All".to_owned(),
                unread: 5,
            },
            FolderView {
                id: 1,
                title: "Work".to_owned(),
                unread: 2,
            },
            FolderView {
                id: 2,
                title: "Archive".to_owned(),
                unread: 0,
            },
        ],
        chats: vec![
            ChatView {
                id: ChatId(100),
                title: "Saved Messages".to_owned(),
                preview: "Intuigram design notes".to_owned(),
                status: String::new(),
                unread: 0,
                pinned: true,
                can_pin_messages: true,
                kind: ChatKind::SavedMessages,
                folders: vec![0],
            },
            ChatView {
                id: ChatId(101),
                title: "Intuigram Contributors".to_owned(),
                preview: "The dense layout feels right.".to_owned(),
                status: String::new(),
                unread: 3,
                pinned: true,
                can_pin_messages: true,
                kind: ChatKind::Supergroup,
                folders: vec![0, 1],
            },
            ChatView {
                id: ChatId(102),
                title: "Terminal Friends".to_owned(),
                preview: "Ship the runnable slice!".to_owned(),
                status: String::new(),
                unread: 2,
                pinned: false,
                can_pin_messages: true,
                kind: ChatKind::Private,
                folders: vec![0],
            },
        ],
        messages: fixture_messages(),
        pinned_messages: Vec::new(),
        drafts: Vec::new(),
        histories: Vec::new(),
    }
}

#[cfg(test)]
fn fixture_messages() -> Vec<MessageView> {
    vec![
        MessageView {
            id: MessageId(1),
            sender: "Intuigram".to_owned(),
            body: "Welcome. This is the live terminal UI, backed by the single-owner app loop."
                .to_owned(),
            timestamp: "09:41".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Read,
            reply_to: None,
            details: MessageDetails::default(),
        },
        MessageView {
            id: MessageId(2),
            sender: "You".to_owned(),
            body: "Dense, focus-driven, and no keyboard modes.".to_owned(),
            timestamp: "09:42".to_owned(),
            direction: MessageDirection::Outgoing,
            delivery: DeliveryState::Read,
            reply_to: Some(MessageId(1)),
            details: MessageDetails::default(),
        },
        MessageView {
            id: MessageId(3),
            sender: "Intuigram".to_owned(),
            body: "Press ? for exhaustive context help. Type or paste in any open Chat.".to_owned(),
            timestamp: "09:43".to_owned(),
            direction: MessageDirection::Incoming,
            delivery: DeliveryState::Sent,
            reply_to: None,
            details: MessageDetails::default(),
        },
    ]
}
#[cfg(test)]
use super::*;
