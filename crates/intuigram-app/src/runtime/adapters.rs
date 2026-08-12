use super::*;

pub(crate) trait ApplicationBackend: Clone + 'static {
    async fn execute(
        &self,
        effect: AdapterEffect,
        peers: intuigram_telegram::PeerDirectory,
    ) -> Result<BackendOutput>;

    fn poll_background(&self, _cx: &mut std::task::Context<'_>) -> Poll<Result<BackendOutput>> {
        Poll::Pending
    }

    fn begin_shutdown(&self) {}

    async fn shutdown(self) -> Result<()> {
        Ok(())
    }
}

pub(crate) struct BackendOutput {
    pub(crate) event: Option<AdapterEvent>,
    pub(crate) telegram_update: Option<intuigram_telegram::LiveEvent>,
    pub(crate) peers: intuigram_telegram::PeerDirectory,
}

impl BackendOutput {
    pub(crate) fn event(event: Option<AdapterEvent>) -> Self {
        Self {
            event,
            telegram_update: None,
            peers: intuigram_telegram::PeerDirectory::default(),
        }
    }

    pub(crate) fn telegram_update(update: intuigram_telegram::LiveEvent) -> Self {
        Self {
            event: None,
            telegram_update: Some(update),
            peers: intuigram_telegram::PeerDirectory::default(),
        }
    }
}

impl Backend {
    pub(crate) async fn execute_with_peers(
        &mut self,
        effect: AdapterEffect,
        peers: intuigram_telegram::PeerDirectory,
    ) -> Result<BackendOutput> {
        self.client.merge_peers(peers);
        let AdapterEffect {
            effect,
            random_id,
            cancellation,
        } = effect;
        match effect {
            Effect::SetMessagePinned {
                chat,
                message,
                pinned,
            } => self.set_message_pinned(chat, message, pinned).await,
            effect => Self::execute(
                self,
                AdapterEffect {
                    effect,
                    random_id,
                    cancellation,
                },
            )
            .await
            .map(BackendOutput::event),
        }
    }
}

impl ApplicationBackend for ActorSession {
    async fn execute(
        &self,
        effect: AdapterEffect,
        peers: intuigram_telegram::PeerDirectory,
    ) -> Result<BackendOutput> {
        self.execute(effect, peers).await
    }

    fn poll_background(&self, cx: &mut std::task::Context<'_>) -> Poll<Result<BackendOutput>> {
        self.poll_background(cx)
    }

    fn begin_shutdown(&self) {
        self.begin_shutdown();
    }

    async fn shutdown(self) -> Result<()> {
        self.shutdown().await
    }
}

pub(crate) trait ApplicationEvents {
    fn poll_next_event(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<intuigram_tui::Result<crossterm::event::Event>>;
}

impl ApplicationEvents for TerminalEvents {
    fn poll_next_event(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<intuigram_tui::Result<crossterm::event::Event>> {
        Self::poll_next_event(self, cx)
    }
}

pub(crate) trait ApplicationAdapterEvents {
    fn poll_adapter_event(&mut self, cx: &mut std::task::Context<'_>)
    -> Poll<Result<AdapterBatch>>;

    fn submit_update(&mut self, _update: intuigram_telegram::LiveEvent) {
        panic!("this adapter event source cannot commit an RPC-returned Telegram update")
    }
}

pub(crate) trait WorkerAdapterEvents {
    fn poll_worker_event(&mut self, cx: &mut std::task::Context<'_>) -> Poll<Result<WorkerBatch>>;

    fn close(&mut self);
}

pub(crate) struct AdapterBatch {
    pub(crate) event: Option<AdapterEvent>,
    pub(crate) peers: intuigram_telegram::PeerDirectory,
}

pub(crate) struct WorkerBatch {
    pub(crate) batch: AdapterBatch,
    pub(crate) delivered: Option<SubmissionCompletion>,
}

impl WorkerAdapterEvents for BackendEvents {
    fn close(&mut self) {
        self.submitted_updates.close();
    }

    fn poll_worker_event(&mut self, cx: &mut std::task::Context<'_>) -> Poll<Result<WorkerBatch>> {
        loop {
            if self.stopped {
                return Poll::Pending;
            }
            if self.pending.is_none()
                && let Some(event) = self.pending_events.pop_front()
            {
                return Poll::Ready(Ok(WorkerBatch {
                    batch: AdapterBatch {
                        event: Some(event),
                        peers: intuigram_telegram::PeerDirectory::default(),
                    },
                    delivered: self
                        .pending_events
                        .is_empty()
                        .then(|| self.pending_submission.take())
                        .flatten(),
                }));
            }
            if let Some(request) = &mut self.pending {
                match Pin::new(request).poll(cx) {
                    Poll::Ready(Ok(update)) => {
                        self.pending = None;
                        self.pending_events.extend(update.events);
                        let batch = WorkerBatch {
                            batch: AdapterBatch {
                                event: self.pending_events.pop_front(),
                                peers: update.peers,
                            },
                            delivered: self
                                .pending_events
                                .is_empty()
                                .then(|| self.pending_submission.take())
                                .flatten(),
                        };
                        return Poll::Ready(Ok(batch));
                    }
                    Poll::Ready(Err(source)) => {
                        self.pending = None;
                        let error = Error::CommitTelegramUpdate { source };
                        if let Some(submission) = self.pending_submission.take() {
                            submission.complete(Err(Box::new(error)));
                            self.stopped = true;
                            return Poll::Pending;
                        }
                        return Poll::Ready(Err(error));
                    }
                    Poll::Pending => return Poll::Pending,
                }
            }
            if !self.deferred_updates.is_empty() && self.retry_deferred_update()? {
                continue;
            }
            if self.queued_submission.is_none()
                && let Poll::Ready(Some(submission)) = self.submitted_updates.poll_pop(cx)
            {
                self.queued_submission = Some(QueuedSubmission {
                    submission,
                    preceding_live_updates: self
                        .updates
                        .buffered_len()
                        .saturating_add(self.deferred_updates.len()),
                });
            }
            if self.deferred_updates.is_empty()
                && self
                    .queued_submission
                    .as_ref()
                    .is_some_and(QueuedSubmission::is_ready)
            {
                let queued = self
                    .queued_submission
                    .take()
                    .expect("a checked queued submission remains present");
                let submission = queued.submission;
                match self.committer.commit(submission.update) {
                    Ok(request) => {
                        self.pending = Some(request);
                        self.pending_submission = Some(submission.committed);
                    }
                    Err(source) => {
                        let error = Error::CommitTelegramUpdate { source };
                        submission.committed.complete(Err(Box::new(error)));
                        self.stopped = true;
                        return Poll::Pending;
                    }
                }
                continue;
            }
            // A submission snapshots the already-buffered live update count.
            // Drain exactly that prefix before the RPC result, so later live
            // traffic cannot starve the durability acknowledgement.
            match Pin::new(&mut self.updates).poll_next(cx) {
                Poll::Ready(Some(Ok(event))) => {
                    self.begin_live_update(event)?;
                    continue;
                }
                Poll::Ready(Some(Err(source))) => {
                    return Poll::Ready(Err(Error::Telegram { source }));
                }
                Poll::Ready(None) => {
                    return Poll::Ready(Err(Error::TelegramUpdatesClosed));
                }
                Poll::Pending => {}
            }
            if let Some(error) = self.poll_gap_timeout(cx) {
                return Poll::Ready(Err(error));
            }
            return Poll::Pending;
        }
    }
}
