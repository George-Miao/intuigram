use intuigram_app::{ChatId, MessageId, MessageView, TextEntity};

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

    LoadThread {
        chat: ChatId,
        root: MessageId,
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

    SendText {
        label: String,
        chat: ChatId,
        text: String,
        entities: Option<Vec<TextEntity>>,
        link_preview: Option<bool>,
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
            Self::LoadThread { chat, root, .. } => {
                format!("load Thread {} in Chat {}", root.0, chat.0)
            }
            Self::ReadThread { chat, root, max_id } => format!(
                "read Thread {} in Chat {} through Message {}",
                root.0, chat.0, max_id.0
            ),
            Self::ReadHistory { chat, max_id, .. } => {
                format!("read Chat {} through Message {}", chat.0, max_id.0)
            }
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
            Self::Reconnect => "reconnect".to_owned(),
        }
    }
}
