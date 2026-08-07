use std::collections::HashMap;

use grammers_tl_types as tl;
use intuigram_app::ChatId;
use snafu::OptionExt as _;

use super::error::{PeerUnavailableSnafu, Result};
use super::message_normalization::mark_channel_id;

/// Telegram cloud-peer address required to invoke operations for a Chat.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PeerAddress {
    /// The authenticated Account's Saved Messages Chat.
    SelfUser { chat: ChatId },

    /// A human or bot user with its Account-scoped access hash.
    User { id: i64, access_hash: i64 },

    /// A legacy Basic Group, which does not require an access hash.
    BasicGroup { id: i64 },

    /// A Supergroup, Gigagroup, or Channel with its Account-scoped access hash.
    Channel { id: i64, access_hash: i64 },
}

impl PeerAddress {
    const fn chat_id(&self) -> ChatId {
        match self {
            Self::SelfUser { chat } => *chat,
            Self::User { id, .. } => ChatId(*id),
            Self::BasicGroup { id } => ChatId(-*id),
            Self::Channel { id, .. } => ChatId(mark_channel_id(*id)),
        }
    }

    fn input_peer(&self) -> tl::enums::InputPeer {
        match self {
            Self::SelfUser { .. } => tl::enums::InputPeer::PeerSelf,
            Self::User { id, access_hash } => tl::types::InputPeerUser {
                user_id: *id,
                access_hash: *access_hash,
            }
            .into(),
            Self::BasicGroup { id } => tl::types::InputPeerChat { chat_id: *id }.into(),
            Self::Channel { id, access_hash } => tl::types::InputPeerChannel {
                channel_id: *id,
                access_hash: *access_hash,
            }
            .into(),
        }
    }
}

/// Opaque operation addresses learned from Telegram cloud-peer entities.
#[derive(Clone, Debug, Default)]
pub struct PeerDirectory {
    peers: HashMap<ChatId, PeerAddress>,
}

impl PeerDirectory {
    /// Reports whether an operation address is known for a Chat.
    #[must_use]
    pub fn contains(&self, chat: ChatId) -> bool {
        self.peers.contains_key(&chat)
    }

    /// Adds newer operation addresses to this directory.
    pub fn merge(&mut self, other: Self) {
        self.peers.extend(other.peers);
    }

    /// Adds or replaces one operation address.
    pub fn insert(&mut self, peer: PeerAddress) {
        self.peers.insert(peer.chat_id(), peer);
    }

    pub(super) fn update(&mut self, chats: &[tl::enums::Chat], users: &[tl::enums::User]) {
        for user in users {
            let tl::enums::User::User(user) = user else {
                continue;
            };
            let peer = if user.is_self {
                Some(PeerAddress::SelfUser {
                    chat: ChatId(user.id),
                })
            } else {
                user.access_hash.map(|access_hash| PeerAddress::User {
                    id: user.id,
                    access_hash,
                })
            };
            if let Some(peer) = peer {
                self.insert(peer);
            }
        }
        for chat in chats {
            let (id, peer) = match chat {
                tl::enums::Chat::Chat(chat) => (
                    ChatId(-chat.id),
                    Some(PeerAddress::BasicGroup { id: chat.id }),
                ),
                tl::enums::Chat::Channel(channel) => (
                    ChatId(mark_channel_id(channel.id)),
                    channel.access_hash.map(|access_hash| PeerAddress::Channel {
                        id: channel.id,
                        access_hash,
                    }),
                ),
                tl::enums::Chat::Forbidden(chat) => (
                    ChatId(-chat.id),
                    Some(PeerAddress::BasicGroup { id: chat.id }),
                ),
                tl::enums::Chat::ChannelForbidden(channel) => (
                    ChatId(mark_channel_id(channel.id)),
                    Some(PeerAddress::Channel {
                        id: channel.id,
                        access_hash: channel.access_hash,
                    }),
                ),
                tl::enums::Chat::Empty(_) => continue,
            };
            if let Some(peer) = peer {
                self.peers.insert(id, peer);
            }
        }
    }

    pub(crate) fn resolve(&self, chat: ChatId) -> Result<tl::enums::InputPeer> {
        self.peers
            .get(&chat)
            .map(PeerAddress::input_peer)
            .context(PeerUnavailableSnafu { chat_id: chat.0 })
    }
}
