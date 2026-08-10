use intuigram_lib::{
    AvatarRef, ChatId, GeoPointView, MessageId, MessageView, PlaceView, SavedDialogView,
    SpecializedRefreshTarget, TextEntity, TopicView,
};

#[derive(Clone, Debug)]
pub(super) enum ExpectedCommand {
    LoadHistory {
        chat: ChatId,
        status: Option<String>,
        messages: Vec<MessageView>,
        pinned_messages: Vec<MessageView>,
    },

    FailLoadHistory {
        chat: ChatId,
        reason: String,
    },

    LoadMediaPreview {
        chat: ChatId,
        message: MessageId,
    },

    LoadAvatar {
        avatar: AvatarRef,
    },

    LoadThread {
        chat: ChatId,
        root: MessageId,
        messages: Vec<MessageView>,
    },

    LoadTopics {
        chat: ChatId,
        topics: Vec<TopicView>,
    },

    LoadSavedDialogs {
        chat: ChatId,
        dialogs: Vec<SavedDialogView>,
    },

    LoadSavedHistory {
        chat: ChatId,
        peer: ChatId,
        messages: Vec<MessageView>,
    },

    ReadThread {
        chat: ChatId,
        root: MessageId,
        max_id: MessageId,
    },

    ReadHistory {
        chat: ChatId,
        max_id: MessageId,
        acknowledge: bool,
    },

    ReadSavedHistory {
        chat: ChatId,
        saved_peer: ChatId,
        max_id: MessageId,
        acknowledge: bool,
    },

    SendText {
        label: String,
        chat: ChatId,
        text: String,
        entities: Option<Vec<TextEntity>>,
        link_preview: Option<bool>,
        reply_to: Option<MessageId>,
        thread_root: Option<MessageId>,
    },

    SendSavedText {
        label: String,
        chat: ChatId,
        saved_peer: ChatId,
        text: String,
        reply_to: Option<MessageId>,
        thread_root: Option<MessageId>,
    },

    SendPoll {
        chat: ChatId,
        question: String,
        options: Vec<String>,
        reply_to: Option<MessageId>,
        thread_root: Option<MessageId>,
    },

    SearchPlaces {
        chat: ChatId,
        query: String,
        near: Option<GeoPointView>,
        places: Vec<PlaceView>,
    },

    SendLocation {
        chat: ChatId,
        point: GeoPointView,
        reply_to: Option<MessageId>,
        thread_root: Option<MessageId>,
    },

    SendVenue {
        chat: ChatId,
        venue: PlaceView,
        reply_to: Option<MessageId>,
        thread_root: Option<MessageId>,
    },

    EditMessage {
        chat: ChatId,
        message: MessageId,
        text: String,
        entities: Option<Vec<TextEntity>>,
        attachments: Option<Vec<String>>,
        updated: MessageView,
    },

    DeleteMessages {
        chat: ChatId,
        messages: Vec<MessageId>,
    },

    ForwardMessages {
        source: ChatId,
        destination: ChatId,
        messages: Vec<MessageId>,
    },

    ReactMessage {
        chat: ChatId,
        message: MessageId,
        reaction: String,
        updated: MessageView,
    },

    SetMessagePinned {
        chat: ChatId,
        message: MessageId,
        pinned: bool,
        updated: MessageView,
    },

    VotePoll {
        chat: ChatId,
        message: MessageId,
        options: Vec<usize>,
        updated: MessageView,
    },

    RefreshSpecialized {
        chat: ChatId,
        message: MessageId,
        target: SpecializedRefreshTarget,
        updated: MessageView,
    },

    ToggleTodoItem {
        chat: ChatId,
        message: MessageId,
        item: i32,
        completed: bool,
        updated: MessageView,
    },

    AppendTodoItem {
        chat: ChatId,
        message: MessageId,
        title: String,
        updated: MessageView,
    },

    Reconnect,
}

impl ExpectedCommand {
    pub(super) fn describe(&self) -> String {
        match self {
            Self::LoadHistory { chat, .. } => format!("load history for Chat {}", chat.0),
            Self::FailLoadHistory { chat, .. } => {
                format!("fail loading history for Chat {}", chat.0)
            }
            Self::LoadMediaPreview { chat, message } => {
                format!(
                    "load image preview for Message {} in Chat {}",
                    message.0, chat.0
                )
            }
            Self::LoadAvatar { avatar } => {
                format!("load avatar {} for peer {}", avatar.id.0, avatar.peer.0)
            }
            Self::LoadThread { chat, root, .. } => {
                format!("load Thread {} in Chat {}", root.0, chat.0)
            }
            Self::LoadTopics { chat, .. } => format!("load Topics for Chat {}", chat.0),
            Self::LoadSavedDialogs { chat, .. } => {
                format!("load saved dialogs for Chat {}", chat.0)
            }
            Self::LoadSavedHistory { chat, peer, .. } => format!(
                "load Saved Messages history for peer {} in Chat {}",
                peer.0, chat.0
            ),
            Self::ReadThread { chat, root, max_id } => format!(
                "read Thread {} in Chat {} through Message {}",
                root.0, chat.0, max_id.0
            ),
            Self::ReadHistory { chat, max_id, .. } => {
                format!("read Chat {} through Message {}", chat.0, max_id.0)
            }
            Self::ReadSavedHistory {
                chat,
                saved_peer,
                max_id,
                ..
            } => format!(
                "read peer {} in Chat {} through Message {}",
                saved_peer.0, chat.0, max_id.0
            ),
            Self::SendText {
                chat,
                text,
                link_preview,
                reply_to,
                thread_root,
                ..
            } => format!(
                "send {text:?} to Chat {} with link preview {link_preview:?} replying to {:?} in \
                 Thread {:?}",
                chat.0,
                reply_to.map(|message| message.0),
                thread_root.map(|message| message.0)
            ),
            Self::SendSavedText {
                chat,
                saved_peer,
                text,
                reply_to,
                thread_root,
                ..
            } => format!(
                "send {text:?} to peer {} in Chat {} replying to {:?} in Thread {:?}",
                saved_peer.0,
                chat.0,
                reply_to.map(|message| message.0),
                thread_root.map(|message| message.0)
            ),
            Self::SendPoll {
                chat,
                question,
                options,
                reply_to,
                thread_root,
            } => format!(
                "send poll {question:?} with {options:?} to Chat {} replying to {:?} in Thread \
                 {:?}",
                chat.0,
                reply_to.map(|message| message.0),
                thread_root.map(|message| message.0)
            ),
            Self::SearchPlaces {
                chat, query, near, ..
            } => {
                format!(
                    "search for places {query:?} near {near:?} in Chat {}",
                    chat.0
                )
            }
            Self::SendLocation {
                chat,
                point,
                reply_to,
                thread_root,
            } => format!(
                "send location {} to Chat {} replying to {reply_to:?} in Thread {thread_root:?}",
                point.coordinates(),
                chat.0,
            ),
            Self::SendVenue {
                chat,
                venue,
                reply_to,
                thread_root,
            } => format!(
                "send venue {:?} to Chat {} replying to {reply_to:?} in Thread {thread_root:?}",
                venue.title, chat.0,
            ),
            Self::EditMessage {
                chat,
                message,
                text,
                ..
            } => format!("edit Message {} in Chat {} to {text:?}", message.0, chat.0),
            Self::DeleteMessages { chat, messages } => format!(
                "delete Messages {:?} from Chat {}",
                messages.iter().map(|message| message.0).collect::<Vec<_>>(),
                chat.0
            ),
            Self::ForwardMessages {
                source,
                destination,
                messages,
            } => format!(
                "forward Messages {:?} from Chat {} to Chat {}",
                messages.iter().map(|message| message.0).collect::<Vec<_>>(),
                source.0,
                destination.0
            ),
            Self::ReactMessage {
                chat,
                message,
                reaction,
                ..
            } => format!(
                "react to Message {} in Chat {} with {reaction:?}",
                message.0, chat.0
            ),
            Self::SetMessagePinned {
                chat,
                message,
                pinned,
                ..
            } => format!(
                "set pinned state of Message {} in Chat {} to {pinned}",
                message.0, chat.0
            ),
            Self::VotePoll {
                chat,
                message,
                options,
                ..
            } => format!(
                "vote for options {options:?} in Message {} of Chat {}",
                message.0, chat.0
            ),
            Self::RefreshSpecialized {
                chat,
                message,
                target,
                ..
            } => format!(
                "refresh {target:?} in Message {} of Chat {}",
                message.0, chat.0
            ),
            Self::ToggleTodoItem {
                chat,
                message,
                item,
                completed,
                ..
            } => format!(
                "set TODO item {item} in Message {} of Chat {} to {completed}",
                message.0, chat.0
            ),
            Self::AppendTodoItem {
                chat,
                message,
                title,
                ..
            } => format!(
                "append TODO item {title:?} to Message {} of Chat {}",
                message.0, chat.0
            ),
            Self::Reconnect => "reconnect".to_owned(),
        }
    }
}
