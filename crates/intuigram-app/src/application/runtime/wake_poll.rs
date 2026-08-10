use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;

use super::super::ApplicationUi;
use super::adapters::{ApplicationAdapterEvents, ApplicationBackend, ApplicationEvents};
use super::types::{ApplicationWake, PendingEffect};

#[derive(Clone, Copy)]
#[repr(usize)]
enum PollSource {
    Adapter,
    Terminal,
    Backend,
    Background,
    Redraw,
    Animation,
}

impl PollSource {
    const ORDER: [Self; 6] = [
        Self::Adapter,
        Self::Terminal,
        Self::Backend,
        Self::Background,
        Self::Redraw,
        Self::Animation,
    ];

    fn offset(self, offset: usize) -> Self {
        Self::ORDER[(self as usize + offset) % Self::ORDER.len()]
    }
}

pub(super) struct WakePoller {
    next: PollSource,
}

pub(super) struct WakePolicy {
    pub(super) poll_adapter: bool,
    pub(super) poll_interaction: bool,
    pub(super) poll_background: bool,
}

pub(super) struct WakeSources<'a, U, E, A, B> {
    pub(super) ui: &'a mut U,
    pub(super) events: &'a mut E,
    pub(super) adapter_events: &'a mut A,
    pub(super) active_effects: &'a mut FuturesUnordered<PendingEffect>,
    pub(super) backend: &'a B,
    pub(super) animation_timer: &'a mut Option<Pin<Box<dyn Future<Output = ()>>>>,
}

impl WakePoller {
    pub(super) const fn new() -> Self {
        Self {
            next: PollSource::Adapter,
        }
    }

    pub(super) fn poll<U, E, A, B>(
        &mut self,
        sources: WakeSources<'_, U, E, A, B>,
        policy: WakePolicy,
        cx: &mut Context<'_>,
    ) -> Poll<ApplicationWake>
    where
        U: ApplicationUi,
        E: ApplicationEvents,
        A: ApplicationAdapterEvents,
        B: ApplicationBackend,
    {
        let WakeSources {
            ui,
            events,
            adapter_events,
            active_effects,
            backend,
            animation_timer,
        } = sources;
        for offset in 0..PollSource::ORDER.len() {
            let source = self.next.offset(offset);
            let wake = match source {
                PollSource::Adapter if policy.poll_adapter => {
                    ready(adapter_events.poll_adapter_event(cx))
                        .map(Box::new)
                        .map(ApplicationWake::Adapter)
                }
                PollSource::Terminal if policy.poll_interaction => {
                    ready(events.poll_next_event(cx)).map(ApplicationWake::Terminal)
                }
                PollSource::Backend => ready(active_effects.poll_next_unpin(cx))
                    .flatten()
                    .map(Box::new)
                    .map(ApplicationWake::Backend),
                PollSource::Background if policy.poll_background => {
                    ready(backend.poll_background(cx))
                        .map(Box::new)
                        .map(ApplicationWake::Background)
                }
                PollSource::Redraw => ready(ui.poll_redraw(cx)).map(ApplicationWake::Redraw),
                PollSource::Animation if policy.poll_interaction => animation_timer
                    .as_mut()
                    .and_then(|timer| ready(timer.as_mut().poll(cx)))
                    .map(|()| ApplicationWake::Animation),
                PollSource::Adapter
                | PollSource::Terminal
                | PollSource::Background
                | PollSource::Animation => None,
            };
            if let Some(wake) = wake {
                self.next = source.offset(1);
                return Poll::Ready(wake);
            }
        }
        Poll::Pending
    }
}

fn ready<T>(poll: Poll<T>) -> Option<T> {
    match poll {
        Poll::Ready(value) => Some(value),
        Poll::Pending => None,
    }
}
