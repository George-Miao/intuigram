use std::collections::{HashMap, HashSet};

use grammers_tl_types as tl;
use intuigram_lib::{AvatarId, AvatarRef, ChatId};
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
    photos: HashMap<ChatId, PeerPhoto>,
    removed_photos: HashSet<ChatId>,
}

#[derive(Clone, Copy, Debug)]
struct PeerPhoto {
    id: i64,
    dc_id: i32,
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
        for peer in other.removed_photos {
            self.photos.remove(&peer);
            self.removed_photos.insert(peer);
        }
        for (peer, photo) in other.photos {
            self.photos.insert(peer, photo);
            self.removed_photos.remove(&peer);
        }
    }

    /// Adds or replaces one operation address.
    pub fn insert(&mut self, peer: PeerAddress) {
        self.peers.insert(peer.chat_id(), peer);
    }

    pub(crate) fn update(&mut self, chats: &[tl::enums::Chat], users: &[tl::enums::User]) {
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
            let id = ChatId(user.id);
            if let Some(tl::enums::UserProfilePhoto::Photo(photo)) = &user.photo {
                self.photos.insert(
                    id,
                    PeerPhoto {
                        id: photo.photo_id,
                        dc_id: photo.dc_id,
                    },
                );
                self.removed_photos.remove(&id);
            } else if !user.min {
                self.photos.remove(&id);
                self.removed_photos.insert(id);
            }
        }
        for chat in chats {
            let (id, peer, photo, authoritative_photo) = match chat {
                tl::enums::Chat::Chat(chat) => (
                    ChatId(-chat.id),
                    Some(PeerAddress::BasicGroup { id: chat.id }),
                    chat_photo(&chat.photo),
                    true,
                ),
                tl::enums::Chat::Channel(channel) => (
                    ChatId(mark_channel_id(channel.id)),
                    channel.access_hash.map(|access_hash| PeerAddress::Channel {
                        id: channel.id,
                        access_hash,
                    }),
                    chat_photo(&channel.photo),
                    !channel.min,
                ),
                tl::enums::Chat::Forbidden(chat) => (
                    ChatId(-chat.id),
                    Some(PeerAddress::BasicGroup { id: chat.id }),
                    None,
                    true,
                ),
                tl::enums::Chat::ChannelForbidden(channel) => (
                    ChatId(mark_channel_id(channel.id)),
                    Some(PeerAddress::Channel {
                        id: channel.id,
                        access_hash: channel.access_hash,
                    }),
                    None,
                    true,
                ),
                tl::enums::Chat::Empty(_) => continue,
            };
            if let Some(peer) = peer {
                self.peers.insert(id, peer);
            }
            if let Some(photo) = photo {
                self.photos.insert(id, photo);
                self.removed_photos.remove(&id);
            } else if authoritative_photo {
                self.photos.remove(&id);
                self.removed_photos.insert(id);
            }
        }
    }

    /// Returns peers with a currently known cloud avatar.
    #[must_use]
    pub fn avatar_peers(&self) -> Vec<AvatarRef> {
        let mut avatars = self
            .photos
            .iter()
            .map(|(peer, photo)| AvatarRef {
                peer: *peer,
                id: AvatarId(photo.id),
            })
            .collect::<Vec<_>>();
        avatars.sort_unstable();
        avatars
    }

    pub(crate) fn avatar_changes(&self) -> Vec<(ChatId, Option<AvatarId>)> {
        let mut changes = self
            .photos
            .iter()
            .map(|(peer, photo)| (*peer, Some(AvatarId(photo.id))))
            .chain(self.removed_photos.iter().copied().map(|peer| (peer, None)))
            .collect::<Vec<_>>();
        changes.sort_unstable_by_key(|(peer, _)| peer.0);
        changes
    }

    pub(super) fn avatar_location(
        &self,
        avatar: AvatarRef,
    ) -> Result<Option<(tl::enums::InputFileLocation, i32)>> {
        let Some(photo) = self
            .photos
            .get(&avatar.peer)
            .filter(|photo| photo.id == avatar.id.0)
        else {
            return Ok(None);
        };
        let input_peer = self.resolve(avatar.peer)?;
        Ok(Some((
            tl::types::InputPeerPhotoFileLocation {
                big: false,
                peer: input_peer,
                photo_id: photo.id,
            }
            .into(),
            photo.dc_id,
        )))
    }

    pub(crate) fn resolve(&self, chat: ChatId) -> Result<tl::enums::InputPeer> {
        self.peers
            .get(&chat)
            .map(PeerAddress::input_peer)
            .context(PeerUnavailableSnafu { chat_id: chat.0 })
    }
}

fn chat_photo(photo: &tl::enums::ChatPhoto) -> Option<PeerPhoto> {
    match photo {
        tl::enums::ChatPhoto::Photo(photo) => Some(PeerPhoto {
            id: photo.photo_id,
            dc_id: photo.dc_id,
        }),
        tl::enums::ChatPhoto::Empty => None,
    }
}
