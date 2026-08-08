use std::cell::RefCell;
use std::rc::Rc;

use compio_actor::{ActorExit, ActorHandle, Cluster, Mailbox};
use intuigram_app::{AdapterEvent, Bootstrap, Effect, MediaPreviewView};
use snafu::ResultExt;

use super::{AdapterEffect, BackendOutput, Error, JoinActorClusterSnafu, Result, RetainedBackend};

mod actor;
mod cancellation;
mod connection;
mod driver;
mod errors;
mod local_effect;
#[cfg(test)]
mod tests;

use actor::{ActorResponse, ExecuteEffect, RestoreRetained, TakeRetained, TelegramActor};
use cancellation::ActorCancellation;
pub(super) use connection::ActorConnection;
use driver::ActorEvents;
use errors::{call_error, deliver_error};

pub(super) struct ConnectedActorSession {
    pub(super) backend: ActorSession,
    pub(super) events: ActorEvents,
    pub(super) peers: intuigram_telegram::PeerDirectory,
    pub(super) bootstrap: Bootstrap,
}

#[derive(Clone)]
pub(super) struct ActorSession {
    owner: Rc<ActorOwner>,
}

struct ActorOwner {
    mailbox: Mailbox<TelegramActor>,
    handle: RefCell<Option<ActorHandle<Error>>>,
    cluster: RefCell<Option<Cluster>>,
    cancellation: ActorCancellation,
    store: intuigram_store::AccountStore,
    local: RefCell<local_effect::State>,
}

impl Drop for ActorOwner {
    fn drop(&mut self) {
        self.mailbox.stop();
    }
}

impl ActorSession {
    pub(super) async fn execute(
        &self,
        effect: AdapterEffect,
        peers: intuigram_telegram::PeerDirectory,
    ) -> Result<BackendOutput> {
        if let Effect::LoadMediaPreview { chat, message } = &effect.effect
            && let Some(image) =
                local_effect::cached_preview(&self.owner.local, *chat, *message).await?
        {
            return Ok(BackendOutput::event(Some(AdapterEvent::MediaPreviewReady(
                MediaPreviewView {
                    chat: *chat,
                    message: *message,
                    image,
                },
            ))));
        }
        if local_effect::handles(&effect.effect) {
            return local_effect::execute(effect.effect, &self.owner.store, &self.owner.local)
                .await;
        }
        let original = effect.effect.clone();
        let attachments = local_effect::attachment_payloads(&original, &self.owner.local).await?;
        let rich_media = local_effect::prepare_rich_media(&original).await?;
        let response = self
            .owner
            .mailbox
            .call(ExecuteEffect {
                effect,
                peers,
                attachments,
                rich_media,
            })
            .await
            .map_err(call_error)?;
        match response {
            ActorResponse::Complete(batch) => {
                local_effect::observe_completion(&original, &batch.event, &self.owner.local);
                Ok(BackendOutput {
                    event: batch.event,
                    telegram_update: None,
                    peers: batch.peers,
                })
            }
            ActorResponse::MediaPreview {
                chat,
                message,
                media,
            } => local_effect::finish_preview(
                &self.owner.local,
                chat,
                message,
                media.map(|media| *media),
            )
            .await
            .map(|event| BackendOutput::event(Some(event))),
            ActorResponse::MediaDownload {
                chat,
                message,
                destination,
                media,
            } => {
                local_effect::finish_download(&self.owner.local, chat, message, destination, *media)
                    .await
                    .map(|event| BackendOutput::event(Some(event)))
            }
            ActorResponse::Failed(error) => Err(*error),
            ActorResponse::Cancelled => Err(Error::TelegramActorCancelled),
        }
    }

    pub(super) fn begin_shutdown(&self) {
        self.owner.cancellation.cancel();
    }

    pub(super) async fn shutdown(self) -> Result<()> {
        let Ok(owner) = Rc::try_unwrap(self.owner) else {
            return Err(Error::TelegramActorMailboxClosed);
        };
        owner.mailbox.stop();
        let handle = owner
            .handle
            .borrow_mut()
            .take()
            .expect("an actor owner retains its handle until shutdown");
        let exit = handle.await;
        let cluster = owner
            .cluster
            .borrow_mut()
            .take()
            .expect("an actor owner retains its cluster until shutdown");
        let joined = cluster.join().await;
        match exit {
            Ok(ActorExit::Stopped) => {}
            Ok(ActorExit::Failed(error)) => {
                return Err(error);
            }
            Err(_) => return Err(Error::TelegramActorExitClosed),
        }
        joined.context(JoinActorClusterSnafu)
    }

    pub(super) async fn take_retained(&self) -> Result<RetainedBackend> {
        let mut retained = self
            .owner
            .mailbox
            .call(TakeRetained)
            .await
            .map_err(call_error)?;
        let local = self.owner.local.borrow();
        retained.attachments.merge(local.attachments.clone());
        retained.downloaded.merge(local.downloaded.clone());
        Ok(retained)
    }

    pub(super) fn restore_retained(&self, retained: RetainedBackend) -> Result<()> {
        {
            let mut local = self.owner.local.borrow_mut();
            local.attachments.merge(retained.attachments.clone());
            local.downloaded.merge(retained.downloaded.clone());
        }
        self.owner
            .mailbox
            .send(RestoreRetained(retained))
            .map_err(deliver_error)
    }
}
