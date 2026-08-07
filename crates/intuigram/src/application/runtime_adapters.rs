use super::*;

pub(super) trait ApplicationBackend: Sized + 'static {
    async fn execute(
        &mut self,
        effect: AdapterEffect,
        peers: intuigram_telegram::PeerDirectory,
    ) -> Result<BackendOutput>;
}

pub(super) struct BackendOutput {
    pub(super) event: Option<AdapterEvent>,
    pub(super) telegram_update: Option<intuigram_telegram::LiveEvent>,
}

impl BackendOutput {
    pub(super) const fn event(event: Option<AdapterEvent>) -> Self {
        Self {
            event,
            telegram_update: None,
        }
    }

    pub(super) fn telegram_update(update: intuigram_telegram::LiveEvent) -> Self {
        Self {
            event: None,
            telegram_update: Some(update),
        }
    }
}

impl ApplicationBackend for Backend {
    async fn execute(
        &mut self,
        effect: AdapterEffect,
        peers: intuigram_telegram::PeerDirectory,
    ) -> Result<BackendOutput> {
        self.client.merge_peers(peers);
        let AdapterEffect { effect, random_id } = effect;
        match effect {
            Effect::SetMessagePinned {
                chat,
                message,
                pinned,
            } => self.set_message_pinned(chat, message, pinned).await,
            effect => Self::execute(self, AdapterEffect { effect, random_id })
                .await
                .map(BackendOutput::event),
        }
    }
}

pub(super) trait ApplicationEvents {
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

pub(super) trait ApplicationAdapterEvents {
    fn poll_adapter_event(&mut self, cx: &mut std::task::Context<'_>)
    -> Poll<Result<AdapterBatch>>;

    fn submit_update(&mut self, _update: intuigram_telegram::LiveEvent) {
        panic!("this adapter event source cannot commit an RPC-returned Telegram update")
    }
}

pub(super) struct AdapterBatch {
    pub(super) event: Option<AdapterEvent>,
    pub(super) peers: intuigram_telegram::PeerDirectory,
}

impl ApplicationAdapterEvents for BackendEvents {
    fn submit_update(&mut self, update: intuigram_telegram::LiveEvent) {
        self.submitted_updates.push_back(update);
    }

    fn poll_adapter_event(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<AdapterBatch>> {
        loop {
            if self.pending.is_none()
                && let Some(event) = self.pending_events.pop_front()
            {
                return Poll::Ready(Ok(AdapterBatch {
                    event: Some(event),
                    peers: intuigram_telegram::PeerDirectory::default(),
                }));
            }
            if let Some(request) = &mut self.pending {
                match Pin::new(request).poll(cx) {
                    Poll::Ready(Ok(update)) => {
                        self.pending = None;
                        self.pending_events.extend(update.events);
                        return Poll::Ready(Ok(AdapterBatch {
                            event: self.pending_events.pop_front(),
                            peers: update.peers,
                        }));
                    }
                    Poll::Ready(Err(source)) => {
                        self.pending = None;
                        return Poll::Ready(Err(Error::CommitTelegramUpdate { source }));
                    }
                    Poll::Pending => return Poll::Pending,
                }
            }
            if let Some(update) = self.submitted_updates.pop_front() {
                match self.committer.commit(update) {
                    Ok(request) => self.pending = Some(request),
                    Err(source) => {
                        return Poll::Ready(Err(Error::CommitTelegramUpdate { source }));
                    }
                }
                continue;
            }
            match Pin::new(&mut self.updates).poll_next(cx) {
                Poll::Ready(Some(Ok(event))) => match self.committer.commit(event) {
                    Ok(request) => self.pending = Some(request),
                    Err(source) => {
                        return Poll::Ready(Err(Error::CommitTelegramUpdate { source }));
                    }
                },
                Poll::Ready(Some(Err(source))) => {
                    return Poll::Ready(Err(Error::Telegram { source }));
                }
                Poll::Ready(None) => {
                    return Poll::Ready(Err(Error::TelegramUpdatesClosed));
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}
