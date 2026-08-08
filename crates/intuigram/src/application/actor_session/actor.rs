use compio::runtime::{JoinHandle, ResumeUnwind};
use compio_actor::{Actor, Call, Handler, Mailbox};
use intuigram_app::{AdapterEvent, Bootstrap, Effect};
use intuigram_store::{AccountRecord, StoreLayout};
use intuigram_telegram::ApplicationCredentials;

use super::super::{
    AdapterBatch, AdapterEffect, AdapterStorage, AttachmentPayload, Backend, BackendEvents,
    BackendOutput, Error, PreparedRichMedia, Result, RetainedBackend, SubmittedUpdates,
    resume_account,
};
use super::cancellation::{ActorCancellation, until_cancelled};
use super::driver::{DriverStop, SessionEvent, run_driver};

pub(super) struct ActorArguments {
    pub(super) credentials: ApplicationCredentials,
    pub(super) layout: StoreLayout,
    pub(super) account: AccountRecord,
    pub(super) storage: AdapterStorage,
    pub(super) startup: flume::Sender<ActorStartup>,
    pub(super) events: flume::Sender<SessionEvent>,
    pub(super) cancellation: ActorCancellation,
}

pub(super) struct ActorStartup {
    pub(super) peers: intuigram_telegram::PeerDirectory,
    pub(super) bootstrap: Bootstrap,
    pub(super) store: intuigram_store::AccountStore,
    pub(super) downloads: intuigram_media::DownloadDirectory,
    pub(super) media_cache: intuigram_media::MediaCache,
}

pub(super) struct TelegramActor;

pub(super) struct TelegramState {
    backend: Backend,
    submitted: SubmittedUpdates,
    driver_stop: DriverStop,
    driver: Option<JoinHandle<()>>,
    events: Option<BackendEvents>,
    output: flume::Sender<SessionEvent>,
    cancellation: ActorCancellation,
}

pub(super) enum ActorResponse {
    Complete(Box<AdapterBatch>),
    MediaPreview {
        chat: intuigram_app::ChatId,
        message: intuigram_app::MessageId,
        media: Option<Box<intuigram_telegram::DownloadedMedia>>,
    },
    MediaDownload {
        chat: intuigram_app::ChatId,
        message: intuigram_app::MessageId,
        destination: Option<String>,
        media: Box<intuigram_telegram::DownloadedMedia>,
    },
    Failed(Box<Error>),
    Cancelled,
}

pub(super) struct ExecuteEffect {
    pub(super) effect: AdapterEffect,
    pub(super) peers: intuigram_telegram::PeerDirectory,
    pub(super) attachments: Vec<(intuigram_app::AttachmentId, AttachmentPayload)>,
    pub(super) rich_media: Option<PreparedRichMedia>,
}

pub(super) struct TakeRetained;

pub(super) struct RestoreRetained(pub(super) RetainedBackend);

impl Actor for TelegramActor {
    type Arguments = ActorArguments;
    type Error = Error;
    type State = TelegramState;

    async fn pre_start(
        &self,
        _myself: &Mailbox<Self>,
        arguments: Self::Arguments,
    ) -> Result<Self::State> {
        let ActorArguments {
            credentials,
            layout,
            account,
            storage,
            startup,
            events: output,
            cancellation,
        } = arguments;
        let (backend, events, peers, bootstrap) = until_cancelled(
            resume_account(credentials, &layout, &account, storage),
            &cancellation,
        )
        .await?;
        let submitted = events.submitted_updates.clone();
        startup
            .send(ActorStartup {
                peers,
                bootstrap,
                store: backend.store.clone(),
                downloads: backend.downloads.clone(),
                media_cache: backend.media_cache.clone(),
            })
            .map_err(|_| Error::TelegramActorStartupClosed)?;
        Ok(TelegramState {
            backend,
            submitted,
            driver_stop: DriverStop::default(),
            driver: None,
            events: Some(events),
            output,
            cancellation,
        })
    }

    async fn post_start(&self, _myself: &Mailbox<Self>, state: &mut Self::State) -> Result<()> {
        let events = state
            .events
            .take()
            .expect("the Telegram event driver starts exactly once");
        let stop = state.driver_stop.clone();
        let output = state.output.clone();
        state.driver = Some(compio::runtime::spawn(run_driver(events, stop, output)));
        Ok(())
    }

    async fn pre_stop(&self, _myself: &Mailbox<Self>, state: &mut Self::State) -> Result<()> {
        state.driver_stop.stop();
        if let Some(driver) = state.driver.take() {
            driver
                .await
                .resume_unwind()
                .ok_or(Error::TelegramActorDriverCancelled)?;
        }
        Ok(())
    }
}

impl Handler<Call<ExecuteEffect, ActorResponse>> for TelegramActor {
    async fn handle(
        &self,
        _myself: &Mailbox<Self>,
        call: Call<ExecuteEffect, ActorResponse>,
        state: &mut Self::State,
    ) -> Result<()> {
        let (request, reply) = call.into_parts();
        for (id, payload) in request.attachments {
            state.backend.attachments.next_id = state.backend.attachments.next_id.max(id.0);
            state.backend.attachments.payloads.insert(id, payload);
        }
        state.backend.client.merge_peers(request.peers.clone());
        let cancellation = state.cancellation.clone();
        let AdapterEffect { effect, random_id } = request.effect;
        let response = match effect {
            Effect::LoadMediaPreview { chat, message } => {
                preview_response(state, &cancellation, chat, message).await
            }
            Effect::DownloadMedia {
                chat,
                message,
                destination,
            } => download_response(state, &cancellation, chat, message, destination).await,
            effect if request.rich_media.is_some() => {
                let prepared = request
                    .rich_media
                    .expect("a checked prepared rich-media payload remains present");
                let result = until_cancelled(
                    state
                        .backend
                        .execute_prepared_rich_media(effect, random_id, prepared),
                    &cancellation,
                )
                .await
                .map(BackendOutput::event);
                actor_response(result, &state.submitted).await
            }
            effect => {
                let result = until_cancelled(
                    state
                        .backend
                        .execute_with_peers(AdapterEffect { effect, random_id }, request.peers),
                    &cancellation,
                )
                .await;
                actor_response(result, &state.submitted).await
            }
        };
        reply.reply(response).ok();
        Ok(())
    }
}

async fn preview_response(
    state: &mut TelegramState,
    cancellation: &ActorCancellation,
    chat: intuigram_app::ChatId,
    message: intuigram_app::MessageId,
) -> ActorResponse {
    let result = until_cancelled(
        async {
            state
                .backend
                .client
                .download_media_preview(chat, message)
                .await
                .map_err(|source| Error::Telegram { source })
        },
        cancellation,
    )
    .await;
    match result {
        Ok(media) => ActorResponse::MediaPreview {
            chat,
            message,
            media: media.map(Box::new),
        },
        Err(Error::Telegram { source }) if !source.is_connection_failure() => {
            ActorResponse::MediaPreview {
                chat,
                message,
                media: None,
            }
        }
        Err(Error::TelegramActorCancelled) => ActorResponse::Cancelled,
        Err(error) => ActorResponse::Failed(Box::new(error)),
    }
}

async fn download_response(
    state: &mut TelegramState,
    cancellation: &ActorCancellation,
    chat: intuigram_app::ChatId,
    message: intuigram_app::MessageId,
    destination: Option<String>,
) -> ActorResponse {
    let result = until_cancelled(
        async {
            state
                .backend
                .client
                .download_media(chat, message)
                .await
                .map_err(|source| Error::Telegram { source })
        },
        cancellation,
    )
    .await;
    match result {
        Ok(media) => ActorResponse::MediaDownload {
            chat,
            message,
            destination,
            media: Box::new(media),
        },
        Err(Error::Telegram { source }) if !source.is_connection_failure() => {
            ActorResponse::Complete(Box::new(AdapterBatch {
                event: Some(AdapterEvent::OperationFailed(source.to_string())),
                peers: intuigram_telegram::PeerDirectory::default(),
            }))
        }
        Err(Error::TelegramActorCancelled) => ActorResponse::Cancelled,
        Err(error) => ActorResponse::Failed(Box::new(error)),
    }
}

async fn actor_response(
    result: Result<BackendOutput>,
    submitted: &SubmittedUpdates,
) -> ActorResponse {
    match result {
        Ok(mut output) => {
            if let Some(update) = output.telegram_update.take() {
                let committed = submitted.push(update);
                match committed.await {
                    Ok(()) => ActorResponse::Complete(Box::new(AdapterBatch {
                        event: None,
                        peers: intuigram_telegram::PeerDirectory::default(),
                    })),
                    Err(error) => ActorResponse::Failed(error),
                }
            } else {
                ActorResponse::Complete(Box::new(AdapterBatch {
                    event: output.event,
                    peers: intuigram_telegram::PeerDirectory::default(),
                }))
            }
        }
        Err(Error::TelegramActorCancelled) => ActorResponse::Cancelled,
        Err(error) => ActorResponse::Failed(Box::new(error)),
    }
}

impl Handler<Call<TakeRetained, RetainedBackend>> for TelegramActor {
    async fn handle(
        &self,
        _myself: &Mailbox<Self>,
        call: Call<TakeRetained, RetainedBackend>,
        state: &mut Self::State,
    ) -> Result<()> {
        call.reply(RetainedBackend {
            attachments: std::mem::take(&mut state.backend.attachments),
            media_library: std::mem::take(&mut state.backend.media_library),
            downloaded: std::mem::take(&mut state.backend.downloaded),
        })
        .ok();
        Ok(())
    }
}

impl Handler<RestoreRetained> for TelegramActor {
    async fn handle(
        &self,
        _myself: &Mailbox<Self>,
        message: RestoreRetained,
        state: &mut Self::State,
    ) -> Result<()> {
        state.backend.attachments.merge(message.0.attachments);
        state.backend.media_library.merge(message.0.media_library);
        state.backend.downloaded.merge(message.0.downloaded);
        Ok(())
    }
}
