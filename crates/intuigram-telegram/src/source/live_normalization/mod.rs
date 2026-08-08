use super::*;

mod cursor;

use cursor::updates_cursors;

pub(crate) struct NormalizedLive {
    pub(crate) events: Vec<AdapterEvent>,
    pub(crate) cursors: Vec<UpdateCursor>,
    pub(crate) peers: PeerDirectory,
}

pub(crate) fn normalize_live_update(
    bytes: &[u8],
    names: &mut HashMap<ChatId, String>,
) -> Result<NormalizedLive> {
    if let Ok(updates) = tl::enums::Updates::from_bytes(bytes) {
        let cursors = updates_cursors(&updates);
        let mut peers = PeerDirectory::default();
        let events = match updates {
            tl::enums::Updates::TooLong | tl::enums::Updates::UpdateShortSentMessage(_) => {
                Vec::new()
            }
            tl::enums::Updates::UpdateShortMessage(message) => {
                vec![short_user_message(message, names)]
            }
            tl::enums::Updates::UpdateShortChatMessage(message) => {
                vec![short_chat_message(message, names)]
            }
            tl::enums::Updates::UpdateShort(update) => normalize_update(update.update, names),
            tl::enums::Updates::Combined(updates) => {
                peers.update(&updates.chats, &updates.users);
                normalize_updates(updates.updates, &updates.chats, &updates.users, names)
            }
            tl::enums::Updates::Updates(updates) => {
                peers.update(&updates.chats, &updates.users);
                normalize_updates(updates.updates, &updates.chats, &updates.users, names)
            }
        };
        return Ok(NormalizedLive {
            events,
            cursors,
            peers,
        });
    }
    let update = tl::enums::Update::from_bytes(bytes).context(DecodeUpdateSnafu)?;
    let cursors = cursor::update_cursors(&update);
    Ok(NormalizedLive {
        events: normalize_update(update, names),
        cursors,
        peers: PeerDirectory::default(),
    })
}

fn short_user_message(
    message: tl::types::UpdateShortMessage,
    names: &HashMap<ChatId, String>,
) -> AdapterEvent {
    let chat = ChatId(message.user_id);
    AdapterEvent::MessageAdded {
        chat,
        message: Box::new(MessageView {
            id: MessageId(i64::from(message.id)),
            sender: short_sender(message.out, names.get(&chat), "Unknown user"),
            body: message.message,
            timestamp: format_timestamp(message.date),
            direction: message_direction(message.out),
            delivery: DeliveryState::Sent,
            reply_to: message.reply_to.as_ref().and_then(reply_message_id),
            details: MessageDetails {
                date_label: format_date(message.date),
                entities: normalize_entities(message.entities.as_deref()),
                ..MessageDetails::default()
            },
        }),
    }
}

fn short_chat_message(
    message: tl::types::UpdateShortChatMessage,
    names: &HashMap<ChatId, String>,
) -> AdapterEvent {
    let chat = ChatId(-message.chat_id);
    let sender = ChatId(message.from_id);
    AdapterEvent::MessageAdded {
        chat,
        message: Box::new(MessageView {
            id: MessageId(i64::from(message.id)),
            sender: short_sender(message.out, names.get(&sender), "Unknown user"),
            body: message.message,
            timestamp: format_timestamp(message.date),
            direction: message_direction(message.out),
            delivery: DeliveryState::Sent,
            reply_to: message.reply_to.as_ref().and_then(reply_message_id),
            details: MessageDetails {
                date_label: format_date(message.date),
                entities: normalize_entities(message.entities.as_deref()),
                ..MessageDetails::default()
            },
        }),
    }
}

fn short_sender(out: bool, name: Option<&String>, fallback: &str) -> String {
    if out {
        "You".to_owned()
    } else {
        name.cloned().unwrap_or_else(|| fallback.to_owned())
    }
}

const fn message_direction(out: bool) -> MessageDirection {
    if out {
        MessageDirection::Outgoing
    } else {
        MessageDirection::Incoming
    }
}

fn normalize_updates(
    updates: Vec<tl::enums::Update>,
    chats: &[tl::enums::Chat],
    users: &[tl::enums::User],
    names: &mut HashMap<ChatId, String>,
) -> Vec<AdapterEvent> {
    update_live_names(names, chats, users);
    let mut events = updates
        .into_iter()
        .flat_map(|update| normalize_update(update, names))
        .collect::<Vec<_>>();
    events.extend(chats.iter().filter_map(|chat| {
        let id = match chat {
            tl::enums::Chat::Chat(chat) => ChatId(-chat.id),
            tl::enums::Chat::Channel(chat) if !chat.min => ChatId(mark_channel_id(chat.id)),
            tl::enums::Chat::Channel(_)
            | tl::enums::Chat::Forbidden(_)
            | tl::enums::Chat::ChannelForbidden(_)
            | tl::enums::Chat::Empty(_) => return None,
        };
        Some(AdapterEvent::ChatPinPermissionChanged {
            chat: id,
            can_pin_messages: cloud_chat_can_pin(chat),
        })
    }));
    events
}

pub(crate) fn normalize_update(
    update: tl::enums::Update,
    names: &HashMap<ChatId, String>,
) -> Vec<AdapterEvent> {
    match update {
        tl::enums::Update::NewMessage(update) => {
            normalize_message_update(&update.message, names, false)
        }
        tl::enums::Update::NewChannelMessage(update) => {
            normalize_message_update(&update.message, names, false)
        }
        tl::enums::Update::EditMessage(update) => {
            normalize_message_update(&update.message, names, true)
        }
        tl::enums::Update::EditChannelMessage(update) => {
            normalize_message_update(&update.message, names, true)
        }
        tl::enums::Update::DeleteMessages(update) => vec![AdapterEvent::MessagesDeleted {
            chat: None,
            ids: update
                .messages
                .into_iter()
                .map(|id| MessageId(i64::from(id)))
                .collect(),
        }],
        tl::enums::Update::DeleteChannelMessages(update) => {
            vec![AdapterEvent::MessagesDeleted {
                chat: Some(ChatId(mark_channel_id(update.channel_id))),
                ids: update
                    .messages
                    .into_iter()
                    .map(|id| MessageId(i64::from(id)))
                    .collect(),
            }]
        }
        tl::enums::Update::PinnedMessages(update) => vec![AdapterEvent::MessagesPinChanged {
            chat: marked_peer_id(&update.peer),
            ids: update
                .messages
                .into_iter()
                .map(|id| MessageId(i64::from(id)))
                .collect(),
            pinned: update.pinned,
        }],
        tl::enums::Update::PinnedChannelMessages(update) => {
            vec![AdapterEvent::MessagesPinChanged {
                chat: ChatId(mark_channel_id(update.channel_id)),
                ids: update
                    .messages
                    .into_iter()
                    .map(|id| MessageId(i64::from(id)))
                    .collect(),
                pinned: update.pinned,
            }]
        }
        tl::enums::Update::ReadHistoryInbox(update) => vec![AdapterEvent::HistoryRead {
            chat: marked_peer_id(&update.peer),
            max_id: MessageId(i64::from(update.max_id)),
            outgoing: false,
            unread: u32::try_from(update.still_unread_count).ok(),
        }],
        tl::enums::Update::ReadHistoryOutbox(update) => vec![AdapterEvent::HistoryRead {
            chat: marked_peer_id(&update.peer),
            max_id: MessageId(i64::from(update.max_id)),
            outgoing: true,
            unread: None,
        }],
        tl::enums::Update::ReadChannelInbox(update) => vec![AdapterEvent::HistoryRead {
            chat: ChatId(mark_channel_id(update.channel_id)),
            max_id: MessageId(i64::from(update.max_id)),
            outgoing: false,
            unread: u32::try_from(update.still_unread_count).ok(),
        }],
        tl::enums::Update::ReadChannelOutbox(update) => vec![AdapterEvent::HistoryRead {
            chat: ChatId(mark_channel_id(update.channel_id)),
            max_id: MessageId(i64::from(update.max_id)),
            outgoing: true,
            unread: None,
        }],
        tl::enums::Update::FolderPeers(update) => update
            .folder_peers
            .into_iter()
            .map(|peer| {
                let tl::enums::FolderPeer::Peer(peer) = peer;
                AdapterEvent::ChatArchiveChanged {
                    chat: marked_peer_id(&peer.peer),
                    archived: peer.folder_id == 1,
                }
            })
            .collect(),
        tl::enums::Update::NotifySettings(update) => {
            let tl::enums::NotifyPeer::Peer(peer) = update.peer else {
                return Vec::new();
            };
            vec![AdapterEvent::ChatMuteChanged {
                chat: marked_peer_id(&peer.peer),
                muted: notifications_muted_at(
                    &update.notify_settings,
                    time::OffsetDateTime::now_utc().unix_timestamp(),
                ),
            }]
        }
        _ => Vec::new(),
    }
}

fn normalize_message_update(
    message: &tl::enums::Message,
    names: &HashMap<ChatId, String>,
    edited: bool,
) -> Vec<AdapterEvent> {
    let chat = message_chat_id(message);
    normalize_message(message, names)
        .map(|message| {
            if edited {
                AdapterEvent::MessageUpdated {
                    chat,
                    message: Box::new(message),
                }
            } else {
                AdapterEvent::MessageAdded {
                    chat,
                    message: Box::new(message),
                }
            }
        })
        .into_iter()
        .collect()
}

fn update_live_names(
    names: &mut HashMap<ChatId, String>,
    chats: &[tl::enums::Chat],
    users: &[tl::enums::User],
) {
    for user in users {
        let (id, name) = match user {
            tl::enums::User::User(user) => (user.id, user_display_name(user)),
            tl::enums::User::Empty(user) => (user.id, "Inaccessible user".to_owned()),
        };
        names.insert(ChatId(id), name);
    }
    for chat in chats {
        let (id, title) = match chat {
            tl::enums::Chat::Chat(chat) => (ChatId(-chat.id), chat.title.clone()),
            tl::enums::Chat::Channel(chat) => {
                (ChatId(mark_channel_id(chat.id)), chat.title.clone())
            }
            tl::enums::Chat::Forbidden(chat) => (ChatId(-chat.id), chat.title.clone()),
            tl::enums::Chat::ChannelForbidden(chat) => {
                (ChatId(mark_channel_id(chat.id)), chat.title.clone())
            }
            tl::enums::Chat::Empty(chat) => (ChatId(-chat.id), "Inaccessible group".to_owned()),
        };
        names.insert(id, title);
    }
}
