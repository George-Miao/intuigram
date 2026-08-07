use intuigram_app::{
    Bootstrap, ChatId, ChatKind, ChatView, ConnectionState, DeliveryState, DraftView, FolderView,
    MessageDetails, MessageDirection, MessageId, MessageView,
};

#[derive(Clone, Debug)]
pub struct AccountFixture {
    name: String,
    chats: Vec<ChatView>,
    drafts: Vec<DraftView>,
}

impl AccountFixture {
    #[must_use]
    pub fn with_chat(mut self, chat: ChatView) -> Self {
        self.chats.push(chat);
        self
    }

    #[must_use]
    pub fn with_draft(mut self, chat: i64, text: impl Into<String>) -> Self {
        self.drafts.push(DraftView {
            chat: ChatId(chat),
            thread_root: None,
            text: text.into(),
            reply_to: None,
        });
        self
    }

    pub(crate) fn into_bootstrap(self) -> Bootstrap {
        Bootstrap {
            connection: ConnectionState::Connected,
            account_name: self.name,
            folders: vec![
                FolderView {
                    id: 0,
                    title: "All".to_owned(),
                    unread: 0,
                },
                FolderView {
                    id: -1,
                    title: "Archive".to_owned(),
                    unread: 0,
                },
            ],
            chats: self.chats,
            messages: Vec::new(),
            pinned_messages: Vec::new(),
            drafts: self.drafts,
            histories: Vec::new(),
        }
    }
}

#[must_use]
pub fn account(name: impl Into<String>) -> AccountFixture {
    AccountFixture {
        name: name.into(),
        chats: Vec::new(),
        drafts: Vec::new(),
    }
}

#[must_use]
pub fn chat(id: i64, title: impl Into<String>) -> ChatView {
    ChatView {
        id: ChatId(id),
        title: title.into(),
        preview: String::new(),
        unread: 0,
        pinned: false,
        can_pin_messages: true,
        kind: ChatKind::Private,
        folders: vec![0],
    }
}

#[must_use]
pub fn incoming(id: i64, sender: impl Into<String>, body: impl Into<String>) -> MessageView {
    MessageView {
        id: MessageId(id),
        sender: sender.into(),
        body: body.into(),
        timestamp: "12:00".to_owned(),
        direction: MessageDirection::Incoming,
        delivery: DeliveryState::Read,
        reply_to: None,
        details: MessageDetails::default(),
    }
}

#[must_use]
pub fn sent_message(id: i64, body: impl Into<String>) -> MessageView {
    MessageView {
        id: MessageId(id),
        sender: "You".to_owned(),
        body: body.into(),
        timestamp: "12:01".to_owned(),
        direction: MessageDirection::Outgoing,
        delivery: DeliveryState::Sent,
        reply_to: None,
        details: MessageDetails::default(),
    }
}
