use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::Poll;

use futures_util::task::AtomicWaker;

use super::types::PendingEffect;
use super::*;

#[derive(Clone, Default)]
pub(crate) struct EffectCancellation {
    inner: Arc<State>,
}

#[derive(Default)]
struct State {
    cancelled: AtomicBool,
    waker: AtomicWaker,
}

impl EffectCancellation {
    pub(crate) fn cancel(&self) {
        self.inner.cancelled.store(true, Ordering::Release);
        self.inner.waker.wake();
    }

    pub(crate) fn poll(&self, cx: &mut std::task::Context<'_>) -> Poll<()> {
        if self.is_cancelled() {
            return Poll::Ready(());
        }
        self.inner.waker.register(cx.waker());
        if self.is_cancelled() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Acquire)
    }
}

pub(super) fn cancel_superseded_work(
    active: &mut futures_util::stream::FuturesUnordered<PendingEffect>,
    navigation: Option<&Effect>,
) {
    if !matches!(
        navigation,
        Some(
            Effect::LoadChat { .. }
                | Effect::LoadThread { .. }
                | Effect::LoadTopics(_)
                | Effect::LoadSavedDialogs(_)
                | Effect::LoadSavedHistory { .. }
        )
    ) {
        return;
    }
    for pending in active.iter_mut() {
        if matches!(
            pending.effect(),
            Effect::LoadChat { .. }
                | Effect::LoadThread { .. }
                | Effect::LoadTopics(_)
                | Effect::LoadSavedDialogs(_)
                | Effect::LoadSavedHistory { .. }
                | Effect::LoadMediaPreview { .. }
                | Effect::LoadAvatar { .. }
        ) {
            pending.cancel();
        }
    }
}

pub(super) fn cancelled_media_event(effect: &Effect) -> Option<AdapterEvent> {
    match effect {
        Effect::LoadMediaPreview { chat, message, .. } => Some(AdapterEvent::MediaPreviewFailed {
            chat: *chat,
            message: *message,
        }),
        Effect::LoadAvatar { avatar } => Some(AdapterEvent::AvatarFailed { avatar: *avatar }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use futures_util::StreamExt as _;
    use intuigram_lib::{ChatId, MessageId};

    use super::*;

    #[derive(Clone)]
    struct NeverBackend;

    impl ApplicationBackend for NeverBackend {
        async fn execute(
            &self,
            _effect: AdapterEffect,
            _peers: intuigram_telegram::PeerDirectory,
        ) -> Result<BackendOutput> {
            std::future::pending().await
        }
    }

    #[derive(Clone)]
    struct ActorCancelledBackend;

    impl ApplicationBackend for ActorCancelledBackend {
        async fn execute(
            &self,
            _effect: AdapterEffect,
            _peers: intuigram_telegram::PeerDirectory,
        ) -> Result<BackendOutput> {
            Err(Error::TelegramActorCancelled)
        }
    }

    #[test]
    fn navigation_cancels_stale_history_and_viewport_media() {
        let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");
        runtime.block_on(async {
            let preview = AdapterEffect::new(Effect::LoadMediaPreview {
                chat: ChatId(7),
                message: MessageId(9),
                locator: None,
            })
            .expect("preview effect should be created");
            let mut active = futures_util::stream::FuturesUnordered::new();
            active.push(start_effect(
                NeverBackend,
                preview,
                intuigram_telegram::PeerDirectory::default(),
            ));
            active.push(start_effect(
                NeverBackend,
                AdapterEffect::new(Effect::LoadChat {
                    chat: ChatId(7),
                    selection: None,
                    transcript_anchors: Vec::new(),
                })
                .expect("history effect should be created"),
                intuigram_telegram::PeerDirectory::default(),
            ));

            cancel_superseded_work(
                &mut active,
                Some(&Effect::LoadChat {
                    chat: ChatId(8),
                    selection: None,
                    transcript_anchors: Vec::new(),
                }),
            );

            for _ in 0..2 {
                let completion = active
                    .next()
                    .await
                    .expect("each stale effect should complete");
                assert!(completion.cancelled);
            }
        });
    }

    #[test]
    fn actor_acknowledged_operation_cancellation_is_not_a_fatal_error() {
        let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");
        runtime.block_on(async {
            let mut active = futures_util::stream::FuturesUnordered::new();
            active.push(start_effect(
                ActorCancelledBackend,
                AdapterEffect::new(Effect::LoadChat {
                    chat: ChatId(7),
                    selection: None,
                    transcript_anchors: Vec::new(),
                })
                .expect("history effect should be created"),
                intuigram_telegram::PeerDirectory::default(),
            ));
            cancel_superseded_work(
                &mut active,
                Some(&Effect::LoadChat {
                    chat: ChatId(8),
                    selection: None,
                    transcript_anchors: Vec::new(),
                }),
            );

            let completion = active.next().await.expect("cancellation should complete");
            assert!(completion.cancelled);
        });
    }
}
