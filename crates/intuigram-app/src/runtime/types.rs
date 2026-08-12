pub(crate) struct BackendCompletion {
    pub(crate) effect: AdapterEffect,
    pub(crate) result: Result<BackendOutput>,
    pub(crate) cancelled: bool,
}

pub(crate) struct PendingEffect {
    effect: AdapterEffect,
    future: Pin<Box<dyn Future<Output = Result<BackendOutput>>>>,
}

impl PendingEffect {
    pub(super) fn effect(&self) -> &Effect {
        &self.effect.effect
    }

    pub(super) fn cancel(&self) {
        self.effect.cancellation.cancel();
    }
}

impl Future for PendingEffect {
    type Output = BackendCompletion;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if let Poll::Ready(result) = this.future.as_mut().poll(cx) {
            let cancelled = this.effect.cancellation.is_cancelled()
                && matches!(&result, Err(Error::TelegramActorCancelled));
            return Poll::Ready(BackendCompletion {
                effect: this.effect.clone(),
                result,
                cancelled,
            });
        }
        if this.effect.cancellation.poll(cx).is_ready() {
            return Poll::Ready(BackendCompletion {
                effect: this.effect.clone(),
                result: Ok(BackendOutput::event(None)),
                cancelled: true,
            });
        }
        Poll::Pending
    }
}

pub(crate) enum ApplicationWake {
    Terminal(intuigram_tui::Result<crossterm::event::Event>),
    Adapter(Box<Result<AdapterBatch>>),
    Backend(Box<BackendCompletion>),
    Background(Box<Result<BackendOutput>>),
    Redraw(intuigram_tui::Result<()>),
    Animation,
}

pub(crate) struct DisconnectedApplication<B> {
    pub(crate) app: App,
    pub(crate) backend: B,
    pub(crate) pending_effects: VecDeque<AdapterEffect>,
}

pub(crate) enum ApplicationExit<B> {
    Quit,
    Lifecycle {
        request: AccountLifecycle,
        backend: B,
    },
    Disconnected(Box<DisconnectedApplication<B>>),
}

pub(crate) enum AccountSessionExit {
    Quit,
    Lifecycle(AccountLifecycle),
}

pub(crate) struct ApplicationState {
    pub(crate) app: App,
    pub(crate) update: Update,
    pub(crate) pending_effects: VecDeque<AdapterEffect>,
    pub(crate) peers: intuigram_telegram::PeerDirectory,
    pub(crate) media_limits: intuigram_telegram::MediaLimits,
}

pub(crate) fn connection_failure_reason(error: &Error) -> Option<String> {
    match error {
        Error::Telegram { source } if source.is_connection_failure() => {
            Some(error_lines(error).join(": "))
        }
        Error::CommitTelegramUpdate {
            source: crate::SyncError::UpdateGap { scope },
        } => Some(format!(
            "Telegram synchronization gap in {scope}; reconnect to resume"
        )),
        Error::TelegramUpdatesClosed => Some(error.to_string()),
        _ => None,
    }
}

#[derive(Clone)]
pub(crate) struct AdapterEffect {
    pub(crate) effect: Effect,
    pub(crate) random_id: Option<i64>,
    pub(crate) cancellation: EffectCancellation,
}

impl std::fmt::Debug for AdapterEffect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AdapterEffect")
            .field("effect", &self.effect)
            .field("random_id", &self.random_id)
            .finish_non_exhaustive()
    }
}

impl PartialEq for AdapterEffect {
    fn eq(&self, other: &Self) -> bool {
        self.effect == other.effect && self.random_id == other.random_id
    }
}

impl Eq for AdapterEffect {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NotificationKey {
    identity: String,
    chat: ChatId,
}

impl AdapterEffect {
    pub(crate) fn new(effect: Effect) -> Result<Self> {
        let random_id = if matches!(
            effect,
            Effect::SendMessage { .. }
                | Effect::EditMessage { .. }
                | Effect::SendPoll { .. }
                | Effect::ForwardMessages { .. }
                | Effect::SendLibraryMedia { .. }
                | Effect::SendRichMediaFile { .. }
                | Effect::RecordRichMedia { .. }
                | Effect::SendContact { .. }
                | Effect::SendStaticLocation { .. }
                | Effect::SendVenue { .. }
                | Effect::ScheduledOperation { .. }
        ) {
            let mut bytes = [0_u8; 8];
            getrandom::fill(&mut bytes).context(OperationIdSnafu)?;
            Some(i64::from_le_bytes(bytes))
        } else {
            None
        };
        Ok(Self {
            effect,
            random_id,
            cancellation: EffectCancellation::default(),
        })
    }
}

pub(crate) fn notification_key(effect: &Effect) -> Option<NotificationKey> {
    match effect {
        Effect::Notify { identity, chat } => Some(NotificationKey {
            identity: identity.clone(),
            chat: *chat,
        }),
        _ => None,
    }
}

pub(crate) fn start_effect<B: ApplicationBackend>(
    backend: B,
    effect: AdapterEffect,
    peers: intuigram_telegram::PeerDirectory,
) -> PendingEffect {
    let retained = effect.clone();
    PendingEffect {
        effect: retained,
        future: Box::pin(async move { backend.execute(effect, peers).await }),
    }
}

pub(crate) fn enqueue_effect(
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
    if matches!(
        effect,
        Effect::LoadChat { .. }
            | Effect::LoadThread { .. }
            | Effect::LoadTopics(_)
            | Effect::LoadSavedDialogs(_)
            | Effect::LoadSavedHistory { .. }
    ) {
        pending.retain(|pending| {
            !matches!(
                pending.effect,
                Effect::LoadChat { .. }
                    | Effect::LoadThread { .. }
                    | Effect::LoadTopics(_)
                    | Effect::LoadSavedDialogs(_)
                    | Effect::LoadSavedHistory { .. }
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
