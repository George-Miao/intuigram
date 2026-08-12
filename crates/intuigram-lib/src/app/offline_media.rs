use std::collections::{HashSet, VecDeque};

use super::*;

#[derive(Default)]
pub(super) struct OfflineMedia {
    chats: HashSet<ChatId>,
    active: Option<OfflineMediaTarget>,
    queued: VecDeque<OfflineMediaTarget>,
}

impl OfflineMedia {
    pub(super) fn replace(&mut self, chats: impl IntoIterator<Item = ChatId>) {
        self.chats = chats.into_iter().collect();
        self.active = None;
        self.queued.clear();
    }

    pub(super) fn contains(&self, chat: ChatId) -> bool {
        self.chats.contains(&chat)
    }

    fn set(&mut self, chat: ChatId, keep: bool) {
        if keep {
            self.chats.insert(chat);
        } else {
            self.chats.remove(&chat);
            self.queued.retain(|target| target.chat != chat);
        }
    }
}

impl App {
    pub(super) fn toggle_offline_media(&self) -> Option<Effect> {
        let chat = self.active_chat_id()?;
        Some(Effect::SetChatMediaOffline(OfflineMediaPolicy {
            chat,
            keep: !self.offline_media.contains(chat),
        }))
    }

    pub(super) fn apply_offline_media_event(&mut self, event: AdapterEvent) -> Option<Effect> {
        match event {
            AdapterEvent::ChatMediaOfflineChanged(policy) => {
                self.offline_media.set(policy.chat, policy.keep);
                self.sync_offline_chat_view();
                if policy.keep {
                    self.queue_offline_media(policy.chat);
                    self.request_next_offline_media()
                } else {
                    None
                }
            }
            AdapterEvent::ChatMediaOfflineFailed(failure) => {
                self.view.notice = Some(failure.reason);
                None
            }
            AdapterEvent::MediaCachedOffline(target) => self.complete_offline_media(target, None),
            AdapterEvent::MediaCacheOfflineFailed(failure) => {
                let target = failure.message.map(|message| OfflineMediaTarget {
                    chat: failure.chat,
                    message,
                });
                target.and_then(|target| self.complete_offline_media(target, Some(failure.reason)))
            }
            _ => None,
        }
    }

    pub(super) fn queue_offline_media(&mut self, chat: ChatId) {
        if !self.offline_media.contains(chat) {
            return;
        }
        let active = self.offline_media.active;
        let queued = &self.offline_media.queued;
        let mut seen = HashSet::new();
        let targets = self
            .histories
            .iter()
            .filter(|(key, _)| key.chat == chat)
            .flat_map(|(_, messages)| messages)
            .filter(|message| {
                message
                    .details
                    .media
                    .as_ref()
                    .is_some_and(|media| media.remote_id.is_some())
            })
            .map(|message| OfflineMediaTarget {
                chat,
                message: message.id,
            })
            .filter(|target| Some(*target) != active && !queued.contains(target))
            .filter(|target| seen.insert(*target))
            .collect::<Vec<_>>();
        self.offline_media.queued.extend(targets);
    }

    pub(super) fn queue_all_offline_media(&mut self) {
        let chats = self.offline_media.chats.iter().copied().collect::<Vec<_>>();
        for chat in chats {
            self.queue_offline_media(chat);
        }
    }

    pub(super) fn request_next_offline_media(&mut self) -> Option<Effect> {
        if self.offline_media.active.is_some() {
            return None;
        }
        let target = self.offline_media.queued.pop_front()?;
        self.offline_media.active = Some(target);
        Some(Effect::CacheMediaOffline {
            target,
            locator: self.message_media_locator(target.chat, target.message),
        })
    }

    fn complete_offline_media(
        &mut self,
        target: OfflineMediaTarget,
        failure: Option<String>,
    ) -> Option<Effect> {
        if self.offline_media.active != Some(target) {
            return None;
        }
        self.offline_media.active = None;
        if let Some(reason) = failure {
            self.view.notice = Some(reason);
        }
        self.request_next_offline_media()
            .or_else(|| self.request_next_small_media())
            .or_else(|| self.request_next_background_history())
    }

    pub(super) fn sync_offline_chat_view(&mut self) {
        self.view.offline_chats = self.offline_media.chats.iter().copied().collect();
        self.view.offline_chats.sort_unstable();
    }
}
