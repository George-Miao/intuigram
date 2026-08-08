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
    match effect {
        Effect::Notify { .. }
        | Effect::OpenExternalLink { .. }
        | Effect::ReadClipboard { .. }
        | Effect::SelectAttachment { .. }
        | Effect::OpenDownload { .. } => EffectRoute::LocalIndependent,
        Effect::SaveDraft { .. } | Effect::SaveSelection { .. } => EffectRoute::LocalOrdered,
        _ => EffectRoute::Telegram,
    }
}

#[cfg(test)]
mod tests {
    use intuigram_app::{ChatId, MessageId};

    use super::*;

    #[test]
    fn draft_persistence_stays_ordered_with_message_sends() {
        let route = effect_route(&Effect::SaveDraft {
            chat: ChatId(7),
            thread_root: None,
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
            })
            .runs_independently()
        );
    }
}
