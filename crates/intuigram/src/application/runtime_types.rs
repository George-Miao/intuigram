pub(super) struct BackendCompletion {
    pub(super) effect: AdapterEffect,
    pub(super) result: Result<BackendOutput>,
}

pub(super) type PendingEffect = Pin<Box<dyn Future<Output = BackendCompletion>>>;

pub(super) enum ApplicationWake {
    Terminal(intuigram_tui::Result<crossterm::event::Event>),
    Adapter(Box<Result<AdapterBatch>>),
    Backend(Box<BackendCompletion>),
    Animation,
}

pub(super) struct DisconnectedApplication<B> {
    pub(super) app: App,
    pub(super) backend: B,
    pub(super) pending_effects: VecDeque<AdapterEffect>,
}

pub(super) enum ApplicationExit<B> {
    Quit,
    Lifecycle {
        request: AccountLifecycle,
        backend: B,
    },
    Disconnected(Box<DisconnectedApplication<B>>),
}

pub(super) enum AccountSessionExit {
    Quit,
    Lifecycle(AccountLifecycle),
}

pub(super) struct ApplicationState {
    pub(super) app: App,
    pub(super) update: Update,
    pub(super) pending_effects: VecDeque<AdapterEffect>,
    pub(super) peers: intuigram_telegram::PeerDirectory,
}

pub(super) fn connection_failure_reason(error: &Error) -> Option<String> {
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
pub(super) struct AdapterEffect {
    pub(super) effect: Effect,
    pub(super) random_id: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NotificationKey {
    identity: String,
    chat: ChatId,
}

impl AdapterEffect {
    pub(super) fn new(effect: Effect) -> Result<Self> {
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

pub(super) fn notification_key(effect: &Effect) -> Option<NotificationKey> {
    match effect {
        Effect::Notify { identity, chat } => Some(NotificationKey {
            identity: identity.clone(),
            chat: *chat,
        }),
        _ => None,
    }
}

pub(super) fn start_effect<B: ApplicationBackend>(
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

pub(super) fn enqueue_effect(
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
