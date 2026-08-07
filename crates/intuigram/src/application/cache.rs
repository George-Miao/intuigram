pub(super) fn cached_bootstrap(account_name: String, cached: CachedAccount) -> Bootstrap {
    let chats = cached
        .chats
        .into_iter()
        .map(|chat| ChatView {
            id: ChatId(chat.id),
            title: chat.title,
            preview: chat.preview,
            unread: chat.unread,
            pinned: chat.pinned,
            can_pin_messages: chat.can_pin_messages,
            kind: stored_chat_kind(&chat.kind),
            folders: chat.folders,
        })
        .collect::<Vec<_>>();
    let mut grouped = BTreeMap::<(i64, Option<i64>), Vec<MessageView>>::new();
    for message in cached.messages {
        grouped
            .entry((message.chat_id, message.thread_root))
            .or_default()
            .push(decode_stored_message(message));
    }
    let histories = grouped
        .into_iter()
        .map(|((chat, thread_root), messages)| HistoryView {
            chat: ChatId(chat),
            thread_root: thread_root.map(MessageId),
            messages,
        })
        .collect::<Vec<_>>();
    let mut grouped_pins = BTreeMap::<i64, Vec<MessageView>>::new();
    for message in cached.pinned_messages {
        grouped_pins
            .entry(message.chat_id)
            .or_default()
            .push(decode_stored_message(message));
    }
    let pinned_messages = grouped_pins
        .into_iter()
        .map(|(chat, messages)| HistoryView {
            chat: ChatId(chat),
            thread_root: None,
            messages,
        })
        .collect();
    let messages = chats.first().map_or_else(Vec::new, |active| {
        histories
            .iter()
            .find(|history| history.chat == active.id && history.thread_root.is_none())
            .map_or_else(Vec::new, |history| history.messages.clone())
    });
    Bootstrap {
        connection: intuigram_app::ConnectionState::Connecting,
        account_name,
        folders: cached
            .folders
            .into_iter()
            .map(|folder| FolderView {
                id: folder.id,
                title: folder.title,
                unread: folder.unread,
            })
            .collect(),
        chats,
        messages,
        pinned_messages,
        drafts: cached
            .drafts
            .into_iter()
            .map(|draft| DraftView {
                chat: ChatId(draft.chat_id),
                thread_root: draft.thread_root.map(MessageId),
                text: draft.text,
                reply_to: draft.reply_to.map(MessageId),
            })
            .collect(),
        histories,
    }
}

pub(super) fn stored_chat_kind(kind: &str) -> ChatKind {
    match kind {
        "saved_messages" => ChatKind::SavedMessages,
        "private" => ChatKind::Private,
        "bot" => ChatKind::Bot,
        "basic_group" => ChatKind::BasicGroup,
        "supergroup" => ChatKind::Supergroup,
        "gigagroup" => ChatKind::Gigagroup,
        "channel" => ChatKind::Channel,
        _ => ChatKind::Inaccessible,
    }
}
use super::*;
