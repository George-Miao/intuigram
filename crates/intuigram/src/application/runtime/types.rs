pub(in crate::application) struct BackendCompletion {
    pub(in crate::application) effect: AdapterEffect,
    pub(in crate::application) result: Result<BackendOutput>,
}

pub(in crate::application) type PendingEffect = Pin<Box<dyn Future<Output = BackendCompletion>>>;

pub(in crate::application) enum ApplicationWake {
    Terminal(intuigram_tui::Result<crossterm::event::Event>),
    Adapter(Box<Result<AdapterBatch>>),
    Backend(Box<BackendCompletion>),
    Redraw(intuigram_tui::Result<()>),
    Animation,
}

pub(in crate::application) struct DisconnectedApplication<B> {
    pub(in crate::application) app: App,
    pub(in crate::application) backend: B,
    pub(in crate::application) pending_effects: VecDeque<AdapterEffect>,
}

pub(in crate::application) enum ApplicationExit<B> {
    Quit,
    Lifecycle {
        request: AccountLifecycle,
        backend: B,
    },
    Disconnected(Box<DisconnectedApplication<B>>),
}

pub(in crate::application) enum AccountSessionExit {
    Quit,
    Lifecycle(AccountLifecycle),
}

pub(in crate::application) struct ApplicationState {
    pub(in crate::application) app: App,
    pub(in crate::application) update: Update,
    pub(in crate::application) pending_effects: VecDeque<AdapterEffect>,
    pub(in crate::application) peers: intuigram_telegram::PeerDirectory,
}

pub(in crate::application) fn connection_failure_reason(error: &Error) -> Option<String> {
    match error {
        Error::Telegram { source } if source.is_connection_failure() => {
            Some(error_lines(error).join(": "))
        }
        Error::CommitTelegramUpdate { source } if source.requires_reconnect() => {
            Some(error_lines(error).join(": "))
        }
        Error::TelegramUpdatesClosed => Some(error.to_string()),
        _ => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::application) struct AdapterEffect {
    pub(in crate::application) effect: Effect,
    pub(in crate::application) random_id: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::application) struct NotificationKey {
    identity: String,
    chat: ChatId,
}

impl AdapterEffect {
    pub(in crate::application) fn new(effect: Effect) -> Result<Self> {
        let random_id = if matches!(
            effect,
            Effect::SendMessage { .. }
                | Effect::SendPoll { .. }
                | Effect::ForwardMessages { .. }
                | Effect::SendLibraryMedia { .. }
                | Effect::SendRichMediaFile { .. }
                | Effect::RecordRichMedia { .. }
                | Effect::SendContact { .. }
                | Effect::ScheduledOperation { .. }
        ) {
            let mut bytes = [0_u8; 8];
            getrandom::fill(&mut bytes).context(OperationIdSnafu)?;
            Some(i64::from_le_bytes(bytes))
        } else {
            None
        };
        Ok(Self { effect, random_id })
    }
}

pub(in crate::application) fn notification_key(effect: &Effect) -> Option<NotificationKey> {
    match effect {
        Effect::Notify { identity, chat } => Some(NotificationKey {
            identity: identity.clone(),
            chat: *chat,
        }),
        _ => None,
    }
}

pub(in crate::application) fn start_effect<B: ApplicationBackend>(
    backend: B,
    effect: AdapterEffect,
    peers: intuigram_telegram::PeerDirectory,
) -> PendingEffect {
    Box::pin(async move {
        let retained = effect.clone();
        let result = backend.execute(effect, peers).await;
        BackendCompletion {
            effect: retained,
            result,
        }
    })
}

pub(in crate::application) fn enqueue_effect(
    pending: &mut VecDeque<AdapterEffect>,
    active: &futures_util::stream::FuturesUnordered<PendingEffect>,
    active_notifications: &[NotificationKey],
    effect: Option<Effect>,
) -> Result<bool> {
    let Some(effect) = effect else {
        return Ok(false);
    };
    if effect == Effect::Quit {
        return Ok(true);
    }
    if let Effect::SaveDraft {
        chat, thread_root, ..
    } = &effect
    {
        pending.retain(|pending| {
            !matches!(
                &pending.effect,
                Effect::SaveDraft {
                    chat: pending_chat,
                    thread_root: pending_thread,
                    ..
                } if pending_chat == chat && pending_thread == thread_root
            )
        });
    }
    if matches!(effect, Effect::SaveSelection { .. }) {
        pending.retain(|pending| !matches!(pending.effect, Effect::SaveSelection { .. }));
    }
    if let Effect::Notify { identity, chat } = &effect {
        if active_notifications
            .iter()
            .any(|active| active.identity == *identity && active.chat == *chat)
        {
            return Ok(false);
        }
        pending.retain(|pending| {
            !matches!(
                &pending.effect,
                Effect::Notify {
                    identity: pending_identity,
                    chat: pending_chat,
                } if pending_identity == identity && pending_chat == chat
            )
        });
    }
    if pending.len() + active.len() >= EFFECT_CAPACITY {
        return EffectsFullSnafu {
            capacity: EFFECT_CAPACITY,
        }
        .fail();
    }
    pending.push_back(AdapterEffect::new(effect)?);
    Ok(false)
}
use super::*;
