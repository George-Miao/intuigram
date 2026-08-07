pub(super) struct BackendCompletion<B> {
    pub(super) backend: B,
    pub(super) effect: AdapterEffect,
    pub(super) result: Result<BackendOutput>,
}

pub(super) type PendingEffect<B> = Pin<Box<dyn Future<Output = BackendCompletion<B>>>>;

pub(super) enum ApplicationWake<B> {
    Terminal(intuigram_tui::Result<crossterm::event::Event>),
    Adapter(Result<AdapterBatch>),
    Backend(BackendCompletion<B>),
    Animation,
}

pub(super) struct DisconnectedApplication<B> {
    pub(super) app: App,
    pub(super) backend: B,
    pub(super) pending_effects: VecDeque<AdapterEffect>,
}

pub(super) enum ApplicationExit<B> {
    Quit,
    Disconnected(Box<DisconnectedApplication<B>>),
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

impl AdapterEffect {
    pub(super) fn new(effect: Effect) -> Result<Self> {
        let random_id = if matches!(
            effect,
            Effect::SendMessage { .. } | Effect::SendPoll { .. } | Effect::ForwardMessage { .. }
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

pub(super) fn start_effect<B: ApplicationBackend>(
    mut backend: B,
    effect: AdapterEffect,
    peers: intuigram_telegram::PeerDirectory,
) -> PendingEffect<B> {
    Box::pin(async move {
        let retained = effect.clone();
        let result = backend.execute(effect, peers).await;
        BackendCompletion {
            backend,
            effect: retained,
            result,
        }
    })
}

pub(super) fn enqueue_effect<B>(
    pending: &mut VecDeque<AdapterEffect>,
    active: &Option<PendingEffect<B>>,
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
    if pending.len() + usize::from(active.is_some()) >= EFFECT_CAPACITY {
        return EffectsFullSnafu {
            capacity: EFFECT_CAPACITY,
        }
        .fail();
    }
    pending.push_back(AdapterEffect::new(effect)?);
    Ok(false)
}
use super::*;
