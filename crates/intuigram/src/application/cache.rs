pub(super) fn cached_bootstrap(
    account_name: String,
    notification_identity: String,
    cached: CachedAccount,
) -> Bootstrap {
    let restored_selection = cached.selection.as_ref().map(|selection| SelectionView {
        folder: selection.folder_id,
        chat: selection.chat_id.map(ChatId),
        message: selection.anchor_message_id.map(MessageId),
    });
    let transcript_anchors = cached
        .selection
        .as_ref()
        .map(|selection| {
            selection
                .transcript_anchors
                .iter()
                .map(|anchor| TranscriptAnchorView {
                    chat: ChatId(anchor.chat_id),
                    thread: anchor.thread_root.map(MessageId),
                    message: MessageId(anchor.message_id),
                })
                .collect()
        })
        .unwrap_or_default();
    let folders = cached
        .folders
        .into_iter()
        .map(|folder| FolderView {
            id: folder.id,
            title: folder.title,
            unread: folder.unread,
        })
        .collect::<Vec<_>>();
    let mut chats = cached
        .chats
        .into_iter()
        .map(|chat| ChatView {
            id: ChatId(chat.id),
            title: chat.title,
            preview: chat.preview,
            preview_sender: None,
            preview_sender_peer: None,
            preview_timestamp: String::new(),
            status: chat.status,
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
    for chat in &mut chats {
        let Some(message) = histories
            .iter()
            .find(|history| history.chat == chat.id && history.thread_root.is_none())
            .and_then(|history| history.messages.last())
        else {
            continue;
        };
        chat.preview_sender = Some(message.sender.clone());
        chat.preview_sender_peer = message.details.sender_peer;
        chat.preview_timestamp.clone_from(&message.timestamp);
    }
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
    let active_chat = match restored_selection {
        None => chats.first().map(|chat| chat.id),
        Some(selection) if folders.iter().any(|folder| folder.id == selection.folder) => {
            selection.chat.filter(|chat| {
                chats.iter().any(|candidate| {
                    candidate.id == *chat && candidate.folders.contains(&selection.folder)
                })
            })
        }
        Some(_) => None,
    };
    let messages = active_chat.map_or_else(Vec::new, |active| {
        histories
            .iter()
            .find(|history| history.chat == active && history.thread_root.is_none())
            .map_or_else(Vec::new, |history| history.messages.clone())
    });
    Bootstrap {
        connection: intuigram_app::ConnectionState::Connecting,
        account_name,
        notification_identity,
        muted_chats: Vec::new(),
        offline_chats: cached.offline_chats.into_iter().map(ChatId).collect(),
        accounts: Vec::new(),
        folder_details: Vec::new(),
        restored_selection,
        transcript_anchors,
        folders,
        chats,
        avatar_peers: Vec::new(),
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
