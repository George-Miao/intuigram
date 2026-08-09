use std::collections::VecDeque;

use super::*;

const MAX_QUEUED_PREVIEWS: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PreviewKey {
    pub(super) chat: ChatId,
    pub(super) message: MessageId,
}

#[derive(Default)]
pub(super) struct MediaPreviewLoads {
    active: Option<PreviewKey>,
    queued: VecDeque<PreviewKey>,
}

impl App {
    pub(super) fn store_media_preview(&mut self, preview: MediaPreviewView) {
        self.view.media_previews.retain(|existing| {
            existing.chat != preview.chat || existing.message != preview.message
        });
        self.view.media_previews.push(preview);
    }

    pub(super) fn queue_active_media_previews(&mut self) {
        self.media_preview_loads.queued.clear();
        self.view.media_preview_loads.clear();
        let Some(chat) = self.active_chat_id() else {
            return;
        };
        let candidates = self
            .view
            .messages
            .iter()
            .rev()
            .filter(|message| {
                message.details.media.as_ref().is_some_and(|media| {
                    media.remote_id.is_some()
                        && matches!(
                            media.kind,
                            MediaKind::Photo | MediaKind::Animation | MediaKind::Sticker
                        )
                })
            })
            .map(|message| PreviewKey {
                chat,
                message: message.id,
            })
            .filter(|key| Some(*key) != self.media_preview_loads.active)
            .filter(|key| {
                !self
                    .view
                    .media_previews
                    .iter()
                    .any(|preview| preview.chat == key.chat && preview.message == key.message)
            })
            .filter(|key| {
                !self.view.downloads.iter().any(|download| {
                    download.chat == key.chat
                        && download.message == key.message
                        && download.preview.is_some()
                })
            })
            .take(MAX_QUEUED_PREVIEWS)
            .collect::<VecDeque<_>>();
        self.view.media_preview_loads.extend(
            self.media_preview_loads
                .active
                .filter(|key| key.chat == chat)
                .into_iter()
                .chain(candidates.iter().copied())
                .map(|key| MediaPreviewLoadView {
                    chat: key.chat,
                    message: key.message,
                }),
        );
        self.media_preview_loads.queued = candidates;
    }

    pub(super) fn request_next_media_preview(&mut self) -> Option<Effect> {
        if self.media_preview_loads.active.is_some() {
            return None;
        }
        let key = self.media_preview_loads.queued.pop_front()?;
        self.media_preview_loads.active = Some(key);
        Some(Effect::LoadMediaPreview {
            chat: key.chat,
            message: key.message,
        })
    }

    pub(super) fn complete_media_preview(&mut self, key: PreviewKey) -> Option<Effect> {
        if self.media_preview_loads.active != Some(key) {
            return None;
        }
        self.media_preview_loads.active = None;
        self.view
            .media_preview_loads
            .retain(|loading| loading.chat != key.chat || loading.message != key.message);
        self.request_next_media_preview()
            .or_else(|| self.request_next_avatar())
            .or_else(|| {
                (!self.history_load_is_active())
                    .then(|| self.request_next_background_history())
                    .flatten()
            })
            .or_else(|| self.take_pending_read())
    }
}
