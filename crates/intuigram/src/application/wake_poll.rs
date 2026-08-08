use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;

use super::runtime_adapters::{ApplicationAdapterEvents, ApplicationEvents};
use super::runtime_types::{ApplicationWake, PendingEffect};

#[derive(Clone, Copy)]
#[repr(usize)]
enum PollSource {
    Adapter,
    Terminal,
    Backend,
    Animation,
}

impl PollSource {
    const ORDER: [Self; 4] = [
        Self::Adapter,
        Self::Terminal,
        Self::Backend,
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
}

impl WakePoller {
    pub(super) const fn new() -> Self {
        Self {
            next: PollSource::Adapter,
        }
    }

    pub(super) fn poll<E, A>(
        &mut self,
        events: &mut E,
        adapter_events: &mut A,
        active_effects: &mut FuturesUnordered<PendingEffect>,
        animation_timer: &mut Option<Pin<Box<dyn Future<Output = ()>>>>,
        policy: WakePolicy,
        cx: &mut Context<'_>,
    ) -> Poll<ApplicationWake>
    where
        E: ApplicationEvents,
        A: ApplicationAdapterEvents,
    {
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
                PollSource::Animation if policy.poll_interaction => animation_timer
                    .as_mut()
                    .and_then(|timer| ready(timer.as_mut().poll(cx)))
                    .map(|()| ApplicationWake::Animation),
                PollSource::Adapter | PollSource::Terminal | PollSource::Animation => None,
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
