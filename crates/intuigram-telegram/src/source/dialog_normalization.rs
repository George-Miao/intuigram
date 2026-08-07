/// Normalizes one serialized current-layer Telegram cloud peer into an
/// Intuigram-owned root Chat category.
pub fn normalize_serialized_peer_kind(bytes: &[u8], account_id: Option<i64>) -> Result<ChatKind> {
    let constructor = bytes
        .get(..4)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(u32::from_le_bytes);
    if matches!(
        constructor,
        Some(tl::types::User::CONSTRUCTOR_ID) | Some(tl::types::UserEmpty::CONSTRUCTOR_ID)
    ) {
        let user = tl::enums::User::from_bytes(bytes).context(DecodePeerSnafu)?;
        return Ok(user_chat_kind(&user, account_id));
    }
    let chat = tl::enums::Chat::from_bytes(bytes).context(DecodePeerSnafu)?;
    Ok(cloud_chat_kind(&chat))
}

pub(crate) fn take_login_token_update(connection: &mut Connection) -> bool {
    connection
        .take_updates()
        .iter()
        .any(|update| contains_login_token_update(update))
}

pub(crate) fn contains_login_token_update(bytes: &[u8]) -> bool {
    if let Ok(update) = tl::enums::Update::from_bytes(bytes) {
        return matches!(update, tl::enums::Update::LoginToken);
    }
    tl::enums::Updates::from_bytes(bytes).is_ok_and(|updates| match updates {
        tl::enums::Updates::UpdateShort(update) => {
            matches!(update.update, tl::enums::Update::LoginToken)
        }
        tl::enums::Updates::Combined(updates) => updates
            .updates
            .iter()
            .any(|update| matches!(update, tl::enums::Update::LoginToken)),
        tl::enums::Updates::Updates(updates) => updates
            .updates
            .iter()
            .any(|update| matches!(update, tl::enums::Update::LoginToken)),
        tl::enums::Updates::TooLong
        | tl::enums::Updates::UpdateShortMessage(_)
        | tl::enums::Updates::UpdateShortChatMessage(_)
        | tl::enums::Updates::UpdateShortSentMessage(_) => false,
    })
}

pub(crate) fn normalize_dialog_folders(
    filters: Vec<tl::enums::DialogFilter>,
    chats: &[ChatView],
) -> Vec<FolderView> {
    let mut folders = filters
        .into_iter()
        .map(|filter| match filter {
            tl::enums::DialogFilter::Default => FolderView {
                id: 0,
                title: "All".to_owned(),
                unread: folder_unread(chats, 0),
            },
            tl::enums::DialogFilter::Filter(filter) => FolderView {
                id: filter.id,
                title: text_with_entities(filter.title),
                unread: folder_unread(chats, filter.id),
            },
            tl::enums::DialogFilter::Chatlist(filter) => FolderView {
                id: filter.id,
                title: text_with_entities(filter.title),
                unread: folder_unread(chats, filter.id),
            },
        })
        .collect::<Vec<_>>();
    if !folders.iter().any(|folder| folder.id == 0) {
        folders.insert(
            0,
            FolderView {
                id: 0,
                title: "All".to_owned(),
                unread: folder_unread(chats, 0),
            },
        );
    }
    folders.push(FolderView {
        id: -1,
        title: "Archive".to_owned(),
        unread: folder_unread(chats, -1),
    });
    folders
}

pub(crate) fn folder_unread(chats: &[ChatView], folder: i32) -> u32 {
    chats
        .iter()
        .filter(|chat| chat.folders.contains(&folder))
        .fold(0_u32, |total, chat| total.saturating_add(chat.unread))
}

pub(crate) fn dialog_filter_id(filter: &tl::enums::DialogFilter) -> Option<i32> {
    match filter {
        tl::enums::DialogFilter::Default => None,
        tl::enums::DialogFilter::Filter(filter) => Some(filter.id),
        tl::enums::DialogFilter::Chatlist(filter) => Some(filter.id),
    }
}

pub(crate) fn set_dialog_filter_membership(
    filter: &mut tl::enums::DialogFilter,
    peer: tl::enums::InputPeer,
    included: bool,
) {
    match filter {
        tl::enums::DialogFilter::Default => {}
        tl::enums::DialogFilter::Filter(filter) => {
            filter.pinned_peers.retain(|candidate| candidate != &peer);
            filter.include_peers.retain(|candidate| candidate != &peer);
            filter.exclude_peers.retain(|candidate| candidate != &peer);
            if included {
                filter.include_peers.push(peer);
            } else {
                filter.exclude_peers.push(peer);
            }
        }
        tl::enums::DialogFilter::Chatlist(filter) => {
            filter.pinned_peers.retain(|candidate| candidate != &peer);
            filter.include_peers.retain(|candidate| candidate != &peer);
            if included {
                filter.include_peers.push(peer);
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ChatTraits {
    pub(super) kind: ChatKind,
    pub(crate) can_pin_messages: bool,
    contact: bool,
}

pub(crate) fn chat_traits(
    chats: &[tl::enums::Chat],
    users: &[tl::enums::User],
    account_id: Option<i64>,
) -> HashMap<ChatId, ChatTraits> {
    let mut result = HashMap::new();
    for user in users {
        let contact = matches!(user, tl::enums::User::User(user) if user.contact);
        result.insert(
            ChatId(user.id()),
            ChatTraits {
                kind: user_chat_kind(user, account_id),
                can_pin_messages: !matches!(user, tl::enums::User::Empty(_)),
                contact,
            },
        );
    }
    for chat in chats {
        let id = match chat {
            tl::enums::Chat::Chat(chat) => ChatId(-chat.id),
            tl::enums::Chat::Forbidden(chat) => ChatId(-chat.id),
            tl::enums::Chat::Empty(chat) => ChatId(-chat.id),
            tl::enums::Chat::Channel(channel) => ChatId(mark_channel_id(channel.id)),
            tl::enums::Chat::ChannelForbidden(channel) => ChatId(mark_channel_id(channel.id)),
        };
        result.insert(
            id,
            ChatTraits {
                kind: cloud_chat_kind(chat),
                can_pin_messages: cloud_chat_can_pin(chat),
                contact: false,
            },
        );
    }
    result
}

pub(crate) fn cloud_chat_can_pin(chat: &tl::enums::Chat) -> bool {
    match chat {
        tl::enums::Chat::Chat(chat) => {
            chat.creator
                || chat.admin_rights.as_ref().is_some_and(admin_can_pin)
                || chat
                    .default_banned_rights
                    .as_ref()
                    .is_none_or(|rights| !pin_is_banned(rights))
        }
        tl::enums::Chat::Channel(channel) => {
            !channel.min
                && (channel.creator
                    || channel.admin_rights.as_ref().is_some_and(admin_can_pin)
                    || (!channel.broadcast
                        && channel
                            .banned_rights
                            .as_ref()
                            .is_none_or(|rights| !pin_is_banned(rights))
                        && channel
                            .default_banned_rights
                            .as_ref()
                            .is_none_or(|rights| !pin_is_banned(rights))))
        }
        tl::enums::Chat::Forbidden(_)
        | tl::enums::Chat::ChannelForbidden(_)
        | tl::enums::Chat::Empty(_) => false,
    }
}

fn admin_can_pin(rights: &tl::enums::ChatAdminRights) -> bool {
    let tl::enums::ChatAdminRights::Rights(rights) = rights;
    rights.pin_messages
}

fn pin_is_banned(rights: &tl::enums::ChatBannedRights) -> bool {
    let tl::enums::ChatBannedRights::Rights(rights) = rights;
    rights.pin_messages
}

pub(crate) fn user_chat_kind(user: &tl::enums::User, account_id: Option<i64>) -> ChatKind {
    match user {
        tl::enums::User::User(user) if user.is_self || account_id == Some(user.id) => {
            ChatKind::SavedMessages
        }
        tl::enums::User::User(user) if user.bot => ChatKind::Bot,
        tl::enums::User::User(_) => ChatKind::Private,
        tl::enums::User::Empty(_) => ChatKind::Inaccessible,
    }
}

pub(crate) fn cloud_chat_kind(chat: &tl::enums::Chat) -> ChatKind {
    match chat {
        tl::enums::Chat::Chat(_) => ChatKind::BasicGroup,
        tl::enums::Chat::Channel(channel) if channel.gigagroup => ChatKind::Gigagroup,
        tl::enums::Chat::Channel(channel) if channel.broadcast => ChatKind::Channel,
        tl::enums::Chat::Channel(_) => ChatKind::Supergroup,
        tl::enums::Chat::Forbidden(_)
        | tl::enums::Chat::ChannelForbidden(_)
        | tl::enums::Chat::Empty(_) => ChatKind::Inaccessible,
    }
}

pub(crate) fn dialog_folder_membership(
    dialog: &tl::types::Dialog,
    filters: &[tl::enums::DialogFilter],
    traits: Option<&ChatTraits>,
) -> Vec<i32> {
    let chat = marked_peer_id(&dialog.peer);
    let archived = dialog.folder_id == Some(1);
    let mut memberships = vec![if archived { -1 } else { 0 }];
    for filter in filters {
        let id = match filter {
            tl::enums::DialogFilter::Default => continue,
            tl::enums::DialogFilter::Filter(filter) => {
                let explicitly_excluded = filter_contains_peer(&filter.exclude_peers, chat, traits);
                let explicitly_included = filter_contains_peer(&filter.pinned_peers, chat, traits)
                    || filter_contains_peer(&filter.include_peers, chat, traits);
                let included_by_kind = traits.is_some_and(|traits| match traits.kind {
                    ChatKind::SavedMessages | ChatKind::Private => {
                        (traits.contact && filter.contacts)
                            || (!traits.contact && filter.non_contacts)
                    }
                    ChatKind::Bot => filter.bots,
                    ChatKind::BasicGroup | ChatKind::Supergroup | ChatKind::Gigagroup => {
                        filter.groups
                    }
                    ChatKind::Channel => filter.broadcasts,
                    ChatKind::Inaccessible => false,
                });
                let excluded_by_state = (filter.exclude_archived && archived)
                    || (filter.exclude_read && dialog.unread_count == 0);
                if explicitly_excluded
                    || excluded_by_state
                    || (!explicitly_included && !included_by_kind)
                {
                    continue;
                }
                filter.id
            }
            tl::enums::DialogFilter::Chatlist(filter) => {
                if !filter_contains_peer(&filter.pinned_peers, chat, traits)
                    && !filter_contains_peer(&filter.include_peers, chat, traits)
                {
                    continue;
                }
                filter.id
            }
        };
        memberships.push(id);
    }
    memberships
}

pub(crate) fn filter_contains_peer(
    peers: &[tl::enums::InputPeer],
    chat: ChatId,
    traits: Option<&ChatTraits>,
) -> bool {
    peers.iter().any(|peer| match peer {
        tl::enums::InputPeer::PeerSelf => {
            traits.is_some_and(|traits| traits.kind == ChatKind::SavedMessages)
        }
        tl::enums::InputPeer::User(peer) => ChatId(peer.user_id) == chat,
        tl::enums::InputPeer::Chat(peer) => ChatId(-peer.chat_id) == chat,
        tl::enums::InputPeer::Channel(peer) => ChatId(mark_channel_id(peer.channel_id)) == chat,
        tl::enums::InputPeer::Empty
        | tl::enums::InputPeer::UserFromMessage(_)
        | tl::enums::InputPeer::ChannelFromMessage(_) => false,
    })
}
use super::*;
