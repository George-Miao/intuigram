use std::collections::{HashSet, VecDeque};

use super::*;

const CHAT_WINDOW_RADIUS: usize = 8;
const MAX_QUEUED_AVATARS: usize = 12;

#[derive(Default)]
pub(super) struct AvatarLoads {
    active: Vec<AvatarRef>,
    queued: VecDeque<AvatarRef>,
    failed: HashSet<AvatarRef>,
}

impl App {
    pub(super) fn update_avatar(&mut self, peer: ChatId, id: Option<AvatarId>) -> Option<Effect> {
        match id {
            Some(id) => {
                self.avatar_peers.insert(peer, id);
            }
            None => {
                self.avatar_peers.remove(&peer);
            }
        }
        self.view
            .avatars
            .retain(|loaded| loaded.avatar.peer != peer || id == Some(loaded.avatar.id));
        self.avatar_loads.invalidate(peer);
        self.queue_visible_avatars();
        self.request_next_small_media()
    }

    pub(super) fn queue_visible_avatars(&mut self) {
        let mut peers = Vec::new();
        if let Some(active) = self.view.active_chat {
            let start = active.saturating_sub(CHAT_WINDOW_RADIUS);
            let end = (active + CHAT_WINDOW_RADIUS + 1).min(self.view.chats.len());
            for chat in &self.view.chats[start..end] {
                peers.push(chat.id);
            }
        }
        peers.extend(
            self.view
                .messages
                .iter()
                .rev()
                .filter_map(|message| message.details.sender_peer),
        );
        peers.extend(self.view.saved_dialogs.iter().map(|dialog| dialog.peer));

        let mut unique = HashSet::new();
        self.avatar_loads.queued = peers
            .into_iter()
            .filter_map(|peer| {
                self.avatar_peers
                    .get(&peer)
                    .copied()
                    .map(|id| AvatarRef { peer, id })
            })
            .filter(|avatar| unique.insert(*avatar))
            .filter(|avatar| !self.avatar_loads.active.contains(avatar))
            .filter(|avatar| !self.avatar_loads.failed.contains(avatar))
            .filter(|avatar| {
                !self
                    .view
                    .avatars
                    .iter()
                    .any(|loaded| loaded.avatar == *avatar)
            })
            .take(MAX_QUEUED_AVATARS)
            .collect();
    }

    pub(super) fn request_next_avatar(&mut self) -> Option<Effect> {
        let avatar = self.avatar_loads.queued.pop_front()?;
        self.avatar_loads.active.push(avatar);
        Some(Effect::LoadAvatar { avatar })
    }

    pub(super) fn complete_avatar(
        &mut self,
        avatar_ref: AvatarRef,
        loaded: Option<AvatarView>,
    ) -> Option<Effect> {
        if !self.avatar_loads.active.contains(&avatar_ref) {
            return None;
        }
        self.avatar_loads
            .active
            .retain(|active| *active != avatar_ref);
        let current = self.avatar_peers.get(&avatar_ref.peer) == Some(&avatar_ref.id);
        if let Some(avatar) = loaded.filter(|_| current) {
            self.view
                .avatars
                .retain(|existing| existing.avatar.peer != avatar.avatar.peer);
            self.view.avatars.push(avatar);
        } else if current {
            self.avatar_loads.failed.insert(avatar_ref);
        }
        self.request_next_small_media()
            .or_else(|| self.take_pending_read())
            .or_else(|| self.request_next_background_history())
    }

    pub(super) fn sync_avatar_load_view(&mut self) {
        self.view.avatar_loads.clear();
        self.view.avatar_loads.extend(
            self.avatar_loads
                .active
                .iter()
                .chain(&self.avatar_loads.queued)
                .copied(),
        );
    }
}

impl AvatarLoads {
    pub(super) fn active_len(&self) -> usize {
        self.active.len()
    }

    fn invalidate(&mut self, peer: ChatId) {
        self.queued.retain(|avatar| avatar.peer != peer);
        self.failed.retain(|avatar| avatar.peer != peer);
    }
}
