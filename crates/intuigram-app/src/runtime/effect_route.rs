use intuigram_lib::Effect;

const SMALL_FILE_LIMIT_BYTES: usize = 20 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EffectRoute {
    TelegramControl,
    SmallMedia,
    LargeTransfer,
    LocalOrdered,
    LocalIndependent,
}

impl EffectRoute {
    pub(crate) const fn is_local(self) -> bool {
        matches!(self, Self::LocalOrdered | Self::LocalIndependent)
    }

    #[cfg(test)]
    const fn runs_independently(self) -> bool {
        matches!(self, Self::LocalIndependent)
    }
}

pub(crate) const fn effect_route(effect: &Effect) -> EffectRoute {
    if super::super::outbox::admission::handles(effect)
        || matches!(effect, Effect::ResolveOutbox { .. })
    {
        return EffectRoute::LocalOrdered;
    }
    match effect {
        Effect::LoadMediaPreview { .. } | Effect::LoadAvatar { .. } => EffectRoute::SmallMedia,
        Effect::DownloadMedia {
            locator: Some(locator),
            ..
        }
        | Effect::CacheMediaOffline {
            locator: Some(locator),
            ..
        } if locator.size < SMALL_FILE_LIMIT_BYTES => EffectRoute::SmallMedia,
        Effect::DownloadMedia { .. } | Effect::CacheMediaOffline { .. } => {
            EffectRoute::LargeTransfer
        }
        Effect::Notify { .. }
        | Effect::OpenExternalLink { .. }
        | Effect::ReadClipboard { .. }
        | Effect::PickAttachment { .. }
        | Effect::SelectAttachment { .. }
        | Effect::OpenDownload { .. } => EffectRoute::LocalIndependent,
        Effect::SaveDraft { .. } | Effect::SaveSelection { .. } => EffectRoute::LocalOrdered,
        _ => EffectRoute::TelegramControl,
    }
}

pub(crate) const fn effect_data_center(effect: &Effect) -> Option<i32> {
    match effect {
        Effect::LoadMediaPreview {
            locator: Some(locator),
            ..
        }
        | Effect::DownloadMedia {
            locator: Some(locator),
            ..
        }
        | Effect::CacheMediaOffline {
            locator: Some(locator),
            ..
        } => Some(locator.dc_id),
        _ => None,
    }
}

pub(crate) const fn effect_priority(effect: &Effect) -> u8 {
    match effect {
        Effect::LoadMediaPreview { .. } | Effect::DownloadMedia { .. } => 0,
        Effect::LoadAvatar { .. } => 1,
        Effect::CacheMediaOffline { .. } => 2,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use intuigram_lib::{ChatId, MessageId, OfflineMediaPolicy};

    use super::*;

    #[test]
    fn draft_persistence_stays_ordered_with_message_sends() {
        let route = effect_route(&Effect::SaveDraft {
            chat: ChatId(7),
            thread_root: None,
            saved_peer: None,
            text: "draft".to_owned(),
            reply_to: Some(MessageId(9)),
        });

        assert_eq!(route, EffectRoute::LocalOrdered);
        assert!(!route.runs_independently());
    }

    #[test]
    fn clipboard_work_can_progress_while_telegram_is_busy() {
        assert!(
            effect_route(&Effect::ReadClipboard {
                chat: ChatId(7),
                thread_root: None,
                saved_peer: None,
            })
            .runs_independently()
        );
    }

    #[test]
    fn offline_policy_changes_use_the_control_lane() {
        let route = effect_route(&Effect::SetChatMediaOffline(OfflineMediaPolicy {
            chat: ChatId(7),
            keep: true,
        }));

        assert_eq!(route, EffectRoute::TelegramControl);
    }

    #[test]
    fn previews_and_downloads_use_separate_bounded_lanes() {
        assert_eq!(
            effect_route(&Effect::LoadMediaPreview {
                chat: ChatId(7),
                message: MessageId(9),
                locator: None,
            }),
            EffectRoute::SmallMedia
        );
        assert_eq!(
            effect_route(&Effect::DownloadMedia {
                chat: ChatId(7),
                message: MessageId(9),
                destination: None,
                locator: None,
            }),
            EffectRoute::LargeTransfer
        );
    }

    #[test]
    fn known_file_size_selects_telegrams_small_or_large_lane() {
        let locator = intuigram_lib::MediaLocator {
            dc_id: 4,
            source: intuigram_lib::MediaSource::Document {
                id: 1,
                access_hash: 2,
                file_reference: vec![3],
            },
            name: "small.bin".to_owned(),
            mime_type: "application/octet-stream".to_owned(),
            size: SMALL_FILE_LIMIT_BYTES - 1,
            thumbnails: Vec::new(),
        };
        let small = Effect::DownloadMedia {
            chat: ChatId(7),
            message: MessageId(9),
            destination: None,
            locator: Some(locator.clone()),
        };
        let large = Effect::DownloadMedia {
            chat: ChatId(7),
            message: MessageId(9),
            destination: None,
            locator: Some(intuigram_lib::MediaLocator {
                size: SMALL_FILE_LIMIT_BYTES,
                ..locator
            }),
        };

        assert_eq!(effect_route(&small), EffectRoute::SmallMedia);
        assert_eq!(effect_route(&large), EffectRoute::LargeTransfer);
    }

    #[test]
    fn visible_media_precedes_background_offline_caching() {
        let preview = Effect::LoadMediaPreview {
            chat: ChatId(7),
            message: MessageId(9),
            locator: None,
        };
        let cache = Effect::CacheMediaOffline {
            target: intuigram_lib::OfflineMediaTarget {
                chat: ChatId(7),
                message: MessageId(9),
            },
            locator: None,
        };

        assert!(effect_priority(&preview) < effect_priority(&cache));
    }

    #[test]
    fn outbound_work_uses_the_ordered_admission_lane() {
        let route = effect_route(&Effect::SendMessage {
            chat: ChatId(7),
            text: "durable".to_owned(),
            entities: Vec::new(),
            link_preview: true,
            reply_to: None,
            thread_root: None,
            saved_peer: None,
            attachments: Vec::new(),
            local_id: MessageId(-1),
        });

        assert_eq!(route, EffectRoute::LocalOrdered);
        assert!(!route.runs_independently());
    }
}
