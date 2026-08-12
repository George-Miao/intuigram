use super::*;

const POSSIBLE_GAP_DELAY: Duration = Duration::from_millis(500);

impl BackendEvents {
    pub(super) fn begin_live_update(
        &mut self,
        update: intuigram_telegram::LiveEvent,
    ) -> Result<()> {
        match self
            .committer
            .commit_or_defer(update)
            .context(CommitTelegramUpdateSnafu)?
        {
            CommitProgress::Started(request) => {
                self.pending = Some(request);
                if let Some(queued) = &mut self.queued_submission {
                    queued.observe_live_update();
                }
            }
            CommitProgress::Deferred(update) => {
                if self.deferred_updates.is_empty() {
                    self.gap_timeout = Some(compio::time::sleep(POSSIBLE_GAP_DELAY));
                }
                self.deferred_updates.push_back(update);
            }
        }
        Ok(())
    }

    pub(super) fn retry_deferred_update(&mut self) -> Result<bool> {
        let Some(deferred) = self.deferred_updates.pop_front() else {
            return Ok(false);
        };
        match self
            .committer
            .commit_or_defer(deferred.update)
            .context(CommitTelegramUpdateSnafu)?
        {
            CommitProgress::Started(request) => {
                self.pending = Some(request);
                if let Some(queued) = &mut self.queued_submission {
                    queued.observe_live_update();
                }
                self.gap_timeout = (!self.deferred_updates.is_empty())
                    .then(|| compio::time::sleep(POSSIBLE_GAP_DELAY));
                Ok(true)
            }
            CommitProgress::Deferred(deferred) => {
                self.deferred_updates.push_front(deferred);
                Ok(false)
            }
        }
    }

    pub(super) fn poll_gap_timeout(&mut self, cx: &mut std::task::Context<'_>) -> Option<Error> {
        let timeout = self.gap_timeout.as_mut()?;
        if Pin::new(timeout).poll(cx).is_pending() {
            return None;
        }
        let deferred = self
            .deferred_updates
            .pop_front()
            .expect("a gap timeout exists only while an update is deferred");
        self.gap_timeout = None;
        Some(Error::CommitTelegramUpdate {
            source: SyncError::UpdateGap {
                scope: deferred.scope,
            },
        })
    }
}
