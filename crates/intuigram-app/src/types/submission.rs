use super::*;

pub(crate) struct BackendEvents {
    pub(crate) updates: LiveUpdates,
    pub(crate) committer: UpdateCommitter,
    pub(crate) pending: Option<UpdateCommit>,
    pub(crate) pending_submission: Option<SubmissionCompletion>,
    pub(crate) queued_submission: Option<QueuedSubmission>,
    pub(crate) pending_events: VecDeque<AdapterEvent>,
    pub(crate) submitted_updates: SubmittedUpdates,
    pub(crate) deferred_updates: VecDeque<DeferredUpdate>,
    pub(crate) gap_timeout: Option<compio::time::Sleep>,
    pub(crate) stopped: bool,
}

pub(crate) type SubmissionResult = std::result::Result<(), Box<Error>>;

#[derive(Clone)]
pub(crate) struct SubmissionCompletion {
    state: std::rc::Rc<std::cell::RefCell<SubmissionState>>,
}

pub(crate) struct SubmissionReceipt {
    state: std::rc::Rc<std::cell::RefCell<SubmissionState>>,
}

#[derive(Default)]
struct SubmissionState {
    result: Option<SubmissionResult>,
    waker: Option<std::task::Waker>,
}

#[derive(Clone, Default)]
pub(crate) struct SubmittedUpdates {
    inner: std::rc::Rc<std::cell::RefCell<SubmittedUpdateState>>,
}

#[derive(Default)]
struct SubmittedUpdateState {
    updates: VecDeque<SubmittedUpdate>,
    waker: Option<std::task::Waker>,
    closed: bool,
}

pub(crate) struct SubmittedUpdate {
    pub(crate) update: intuigram_telegram::LiveEvent,
    pub(crate) committed: SubmissionCompletion,
}

pub(crate) struct QueuedSubmission {
    pub(crate) submission: SubmittedUpdate,
    pub(crate) preceding_live_updates: usize,
}

impl QueuedSubmission {
    pub(crate) const fn is_ready(&self) -> bool {
        self.preceding_live_updates == 0
    }

    pub(crate) fn observe_live_update(&mut self) {
        self.preceding_live_updates = self.preceding_live_updates.saturating_sub(1);
    }
}

impl SubmissionCompletion {
    pub(crate) fn complete(self, result: SubmissionResult) {
        let mut state = self.state.borrow_mut();
        state.result = Some(result);
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }
}

impl Future for SubmissionReceipt {
    type Output = SubmissionResult;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.borrow_mut();
        if let Some(result) = state.result.take() {
            return Poll::Ready(result);
        }
        if state
            .waker
            .as_ref()
            .is_none_or(|waker| !waker.will_wake(cx.waker()))
        {
            state.waker = Some(cx.waker().clone());
        }
        Poll::Pending
    }
}

impl SubmittedUpdates {
    pub(crate) fn push(&self, update: intuigram_telegram::LiveEvent) -> SubmissionReceipt {
        let state = std::rc::Rc::new(std::cell::RefCell::new(SubmissionState::default()));
        let committed = SubmissionCompletion {
            state: std::rc::Rc::clone(&state),
        };
        let receipt = SubmissionReceipt { state };
        let mut state = self.inner.borrow_mut();
        if state.closed {
            committed.complete(Err(Box::new(Error::TelegramActorCancelled)));
            return receipt;
        }
        state
            .updates
            .push_back(SubmittedUpdate { update, committed });
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
        receipt
    }

    pub(crate) fn poll_pop(
        &self,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<SubmittedUpdate>> {
        let mut state = self.inner.borrow_mut();
        match state.updates.pop_front() {
            Some(update) => Poll::Ready(Some(update)),
            None => {
                if state
                    .waker
                    .as_ref()
                    .is_none_or(|waker| !waker.will_wake(cx.waker()))
                {
                    state.waker = Some(cx.waker().clone());
                }
                Poll::Pending
            }
        }
    }

    pub(crate) fn close(&self) {
        let mut state = self.inner.borrow_mut();
        state.closed = true;
        for update in state.updates.drain(..) {
            update
                .committed
                .complete(Err(Box::new(Error::TelegramActorCancelled)));
        }
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }
}

impl Drop for BackendEvents {
    fn drop(&mut self) {
        self.submitted_updates.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_the_driver_wakes_pending_commit_waiters() {
        let submitted = SubmittedUpdates::default();
        let committed = submitted.push(intuigram_telegram::LiveEvent {
            events: Vec::new(),
            cursors: Vec::new(),
            peers: intuigram_telegram::PeerDirectory::default(),
        });

        submitted.close();
        let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");
        let result = runtime.block_on(committed);

        assert!(matches!(
            result,
            Err(error) if matches!(*error, Error::TelegramActorCancelled)
        ));
    }

    #[test]
    fn later_live_updates_do_not_extend_a_submission_barrier() {
        let submitted = SubmittedUpdates::default();
        let _receipt = submitted.push(intuigram_telegram::LiveEvent {
            events: Vec::new(),
            cursors: Vec::new(),
            peers: intuigram_telegram::PeerDirectory::default(),
        });
        let mut context = std::task::Context::from_waker(futures_util::task::noop_waker_ref());
        let Poll::Ready(Some(submission)) = submitted.poll_pop(&mut context) else {
            panic!("submission should be queued")
        };
        let mut barrier = QueuedSubmission {
            submission,
            preceding_live_updates: 2,
        };

        barrier.observe_live_update();
        assert!(!barrier.is_ready());
        barrier.observe_live_update();
        assert!(barrier.is_ready());
        barrier.observe_live_update();
        assert!(barrier.is_ready());
    }
}
