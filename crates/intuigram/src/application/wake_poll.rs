use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use super::runtime_adapters::{ApplicationAdapterEvents, ApplicationBackend, ApplicationEvents};
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

impl WakePoller {
    pub(super) const fn new() -> Self {
        Self {
            next: PollSource::Adapter,
        }
    }

    pub(super) fn poll<E, A, B>(
        &mut self,
        events: &mut E,
        adapter_events: &mut A,
        active_effect: &mut Option<PendingEffect<B>>,
        animation_timer: &mut Option<Pin<Box<dyn Future<Output = ()>>>>,
        disconnected: bool,
        cx: &mut Context<'_>,
    ) -> Poll<ApplicationWake<B>>
    where
        E: ApplicationEvents,
        A: ApplicationAdapterEvents,
        B: ApplicationBackend,
    {
        for offset in 0..PollSource::ORDER.len() {
            let source = self.next.offset(offset);
            let wake = match source {
                PollSource::Adapter if !disconnected => {
                    ready(adapter_events.poll_adapter_event(cx)).map(ApplicationWake::Adapter)
                }
                PollSource::Terminal => {
                    ready(events.poll_next_event(cx)).map(ApplicationWake::Terminal)
                }
                PollSource::Backend => active_effect
                    .as_mut()
                    .and_then(|effect| ready(effect.as_mut().poll(cx)))
                    .map(ApplicationWake::Backend),
                PollSource::Animation => animation_timer
                    .as_mut()
                    .and_then(|timer| ready(timer.as_mut().poll(cx)))
                    .map(|()| ApplicationWake::Animation),
                PollSource::Adapter => None,
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
