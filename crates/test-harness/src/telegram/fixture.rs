use intuigram_app::{
    AccountKey, AccountView, Bootstrap, ChatId, ChatKind, ChatView, ConnectionState, DeliveryState,
    DraftView, FolderDetailsView, FolderRulesView, FolderView, MessageDetails, MessageDirection,
    MessageId, MessageView,
};

#[derive(Clone, Debug)]
pub struct AccountFixture {
    name: String,
    id: AccountKey,
    accounts: Vec<AccountView>,
    folders: Vec<FolderView>,
    folder_details: Vec<FolderDetailsView>,
    chats: Vec<ChatView>,
    messages: Vec<MessageView>,
    drafts: Vec<DraftView>,
    muted_chats: Vec<ChatId>,
}

impl AccountFixture {
    #[must_use]
    pub fn with_identity(mut self, id: i64) -> Self {
        self.id = AccountKey(id);
        self.accounts[0].id = self.id;
        self
    }

    #[must_use]
    pub fn with_registered_account(mut self, id: i64, name: impl Into<String>) -> Self {
        self.accounts.push(AccountView {
            id: AccountKey(id),
            display_name: name.into(),
            active: false,
        });
        self
    }

    #[must_use]
    pub fn with_folder(mut self, id: i32, title: impl Into<String>) -> Self {
        let archive = self
            .folders
            .iter()
            .position(|folder| folder.id == -1)
            .unwrap_or(self.folders.len());
        self.folders.insert(
            archive,
            FolderView {
                id,
                title: title.into(),
                unread: 0,
            },
        );
        self.folder_details.push(FolderDetailsView {
            id: intuigram_app::FolderId(id),
            rules: Some(FolderRulesView::default()),
            shareable: true,
        });
        self
    }

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

    #[must_use]
    pub fn with_muted_chat(mut self, chat: i64) -> Self {
        self.muted_chats.push(ChatId(chat));
        self
    }

    /// Seeds the recent history for the first Chat in this Account fixture.
    #[must_use]
    pub fn with_history(mut self, messages: impl IntoIterator<Item = MessageView>) -> Self {
        self.messages = messages.into_iter().collect();
        self
    }

    pub(crate) fn into_bootstrap(self) -> Bootstrap {
        Bootstrap {
            connection: ConnectionState::Connected,
            account_name: self.name,
            notification_identity: "telegram:test".to_owned(),
            muted_chats: self.muted_chats,
            accounts: self.accounts,
            folder_details: self.folder_details,
            restored_selection: None,
            transcript_anchors: Vec::new(),
            folders: self.folders,
            chats: self.chats,
            messages: self.messages,
            pinned_messages: Vec::new(),
            drafts: self.drafts,
            histories: Vec::new(),
        }
    }
}

#[must_use]
pub fn account(name: impl Into<String>) -> AccountFixture {
    let name = name.into();
    AccountFixture {
        name: name.clone(),
        id: AccountKey(1),
        accounts: vec![AccountView {
            id: AccountKey(1),
            display_name: name,
            active: true,
        }],
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
        folder_details: Vec::new(),
        chats: Vec::new(),
        messages: Vec::new(),
        drafts: Vec::new(),
        muted_chats: Vec::new(),
    }
}

#[must_use]
pub fn chat(id: i64, title: impl Into<String>) -> ChatView {
    ChatView {
        id: ChatId(id),
        title: title.into(),
        preview: String::new(),
        status: "last seen recently".to_owned(),
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
