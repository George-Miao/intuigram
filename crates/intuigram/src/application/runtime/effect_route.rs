use intuigram_app::Effect;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::application) enum EffectRoute {
    Telegram,
    LocalOrdered,
    LocalIndependent,
}

impl EffectRoute {
    pub(in crate::application) const fn is_local(self) -> bool {
        !matches!(self, Self::Telegram)
    }

    #[cfg(test)]
    const fn runs_independently(self) -> bool {
        matches!(self, Self::LocalIndependent)
    }
}

pub(in crate::application) const fn effect_route(effect: &Effect) -> EffectRoute {
    if super::super::outbox::admission::handles(effect)
        || matches!(effect, Effect::ResolveOutbox { .. })
    {
        return EffectRoute::LocalOrdered;
    }
    match effect {
        Effect::Notify { .. }
        | Effect::OpenExternalLink { .. }
        | Effect::ReadClipboard { .. }
        | Effect::PickAttachment { .. }
        | Effect::SelectAttachment { .. }
        | Effect::OpenDownload { .. } => EffectRoute::LocalIndependent,
        Effect::SaveDraft { .. } | Effect::SaveSelection { .. } => EffectRoute::LocalOrdered,
        _ => EffectRoute::Telegram,
    }
}

#[cfg(test)]
mod tests {
    use intuigram_app::{ChatId, MessageId, OfflineMediaPolicy};

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
    fn offline_policy_changes_share_the_single_media_request_lane() {
        let route = effect_route(&Effect::SetChatMediaOffline(OfflineMediaPolicy {
            chat: ChatId(7),
            keep: true,
        }));

        assert_eq!(route, EffectRoute::Telegram);
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
