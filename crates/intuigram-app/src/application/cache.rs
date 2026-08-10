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
                    saved_peer: anchor.saved_peer.map(ChatId),
                    message: MessageId(anchor.message_id),
                })
                .collect()
        })
        .unwrap_or_default();
    let outbox = cached.outbox.into_iter().map(outbox_view).collect();
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
            has_topics: chat.has_topics,
            has_direct_messages: chat.has_direct_messages,
            kind: stored_chat_kind(&chat.kind),
            folders: chat.folders,
        })
        .collect::<Vec<_>>();
    let mut grouped_topics = BTreeMap::<i64, Vec<TopicView>>::new();
    for topic in cached.topics {
        grouped_topics
            .entry(topic.chat_id)
            .or_default()
            .push(TopicView {
                id: TopicId(topic.id),
                title: topic.title,
                preview: topic.preview,
                timestamp: topic.timestamp,
                unread: topic.unread,
                pinned: topic.pinned,
                closed: topic.closed,
                hidden: topic.hidden,
                icon_color: topic.icon_color,
                icon_emoji_id: topic.icon_emoji_id,
                top_message: topic.top_message_id.map(MessageId),
                draft: topic.draft_text.map(|text| TopicDraftView {
                    text,
                    reply_to: topic.draft_reply_to.map(MessageId),
                }),
            });
    }
    let topic_lists = grouped_topics
        .into_iter()
        .map(|(chat, topics)| TopicListView {
            chat: ChatId(chat),
            topics,
        })
        .collect();
    let mut grouped_saved_dialogs = BTreeMap::<i64, Vec<SavedDialogView>>::new();
    for dialog in cached.saved_dialogs {
        grouped_saved_dialogs
            .entry(dialog.chat_id)
            .or_default()
            .push(SavedDialogView {
                peer: ChatId(dialog.peer_id),
                title: dialog.title,
                preview: dialog.preview,
                timestamp: dialog.timestamp,
                unread: dialog.unread,
                unread_mark: dialog.unread_mark,
                pinned: dialog.pinned,
                top_message: MessageId(dialog.top_message_id),
                draft: dialog.draft_text.map(|text| SavedDialogDraftView {
                    text,
                    reply_to: dialog.draft_reply_to.map(MessageId),
                }),
            });
    }
    let saved_dialog_lists = grouped_saved_dialogs
        .into_iter()
        .map(|(chat, dialogs)| SavedDialogListView {
            chat: ChatId(chat),
            dialogs,
        })
        .collect();
    let mut grouped = BTreeMap::<(i64, Option<i64>, Option<i64>), Vec<MessageView>>::new();
    for message in cached.messages {
        let chat = message.chat_id;
        let thread = message.thread_root;
        let saved_peer = message.saved_peer;
        let message = decode_stored_message(message);
        grouped
            .entry((chat, None, None))
            .or_default()
            .push(message.clone());
        if thread.is_some() || saved_peer.is_some() {
            grouped
                .entry((chat, thread, saved_peer))
                .or_default()
                .push(message);
        }
    }
    let histories = grouped
        .into_iter()
        .map(|((chat, thread_root, saved_peer), messages)| HistoryView {
            chat: ChatId(chat),
            thread_root: thread_root.map(MessageId),
            saved_peer: saved_peer.map(ChatId),
            messages,
        })
        .collect::<Vec<_>>();
    for chat in &mut chats {
        let Some(message) = histories
            .iter()
            .find(|history| {
                history.chat == chat.id
                    && history.thread_root.is_none()
                    && history.saved_peer.is_none()
            })
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
            saved_peer: None,
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
            .find(|history| {
                history.chat == active
                    && history.thread_root.is_none()
                    && history.saved_peer.is_none()
            })
            .map_or_else(Vec::new, |history| history.messages.clone())
    });
    Bootstrap {
        connection: intuigram_lib::ConnectionState::Connecting,
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
        topic_lists,
        saved_dialog_lists,
        avatar_peers: Vec::new(),
        messages,
        pinned_messages,
        drafts: cached
            .drafts
            .into_iter()
            .map(|draft| DraftView {
                chat: ChatId(draft.chat_id),
                thread_root: draft.thread_root.map(MessageId),
                saved_peer: draft.saved_peer.map(ChatId),
                text: draft.text,
                reply_to: draft.reply_to.map(MessageId),
            })
            .collect(),
        histories,
        outbox,
    }
}

pub(super) fn outbox_view(record: OutboxRecord) -> OutboxItemView {
    let OutboxPayload::V1(payload) = record.payload;
    OutboxItemView {
        key: OutboxKey(record.id.get()),
        chat: ChatId(payload.chat_id),
        local_message: payload.local_message_id.map(MessageId),
        state: match record.state {
            OutboxState::Ready => OutboxStateView::Ready,
            OutboxState::InFlight => OutboxStateView::InFlight,
            OutboxState::CancelRequested => OutboxStateView::CancelRequested,
            OutboxState::Deferred => OutboxStateView::Deferred,
            OutboxState::Failed => OutboxStateView::Failed,
            OutboxState::Conflict => OutboxStateView::Conflict,
            OutboxState::OutcomeUnknown => OutboxStateView::OutcomeUnknown,
            OutboxState::Expired => OutboxStateView::Expired,
            OutboxState::Cancelled => OutboxStateView::Cancelled,
        },
        retryable: matches!(
            record.operation,
            OutboxOperation::Create | OutboxOperation::Send
        ),
        available_at: record.available_at,
        expires_at: record.expires_at,
        last_error: record.last_error,
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
