use std::cell::RefCell;
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Poll;

use compio_actor::Cluster;
use compio_dispatcher::DispatcherBuilder;
use intuigram_store::{AccountRecord, StoreLayout};
use intuigram_telegram::ApplicationCredentials;
use snafu::ResultExt;

use super::super::{
    AdapterStorage, EFFECT_CAPACITY, Error, JoinActorClusterSnafu, Result, StartActorClusterSnafu,
};
use super::actor::{ActorArguments, TelegramActor};
use super::errors::spawn_error;
use super::{ActorCancellation, ActorEvents, ActorOwner, ActorSession, ConnectedActorSession};

pub(in crate::application) struct ActorConnection {
    cancellation: ActorCancellation,
    future: Pin<Box<dyn Future<Output = Result<ConnectedActorSession>>>>,
}

impl Future for ActorConnection {
    type Output = Result<ConnectedActorSession>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        self.future.as_mut().poll(cx)
    }
}

impl ActorConnection {
    pub(super) fn new(
        credentials: ApplicationCredentials,
        layout: StoreLayout,
        account: AccountRecord,
        storage: AdapterStorage,
    ) -> Self {
        let cancellation = ActorCancellation::default();
        Self {
            cancellation: cancellation.clone(),
            future: Box::pin(ActorSession::connect(
                credentials,
                layout,
                account,
                storage,
                cancellation,
            )),
        }
    }

    pub(in crate::application) async fn cancel(mut self) -> Result<()> {
        self.cancellation.cancel();
        match self.future.as_mut().await {
            Ok(connected) => connected.backend.shutdown().await,
            Err(Error::TelegramActorCancelled) => Ok(()),
            Err(error) => Err(error),
        }
    }
}

impl ActorSession {
    pub(in crate::application) fn connection(
        credentials: ApplicationCredentials,
        layout: StoreLayout,
        account: AccountRecord,
        storage: AdapterStorage,
    ) -> ActorConnection {
        ActorConnection::new(credentials, layout, account, storage)
    }

    async fn connect(
        credentials: ApplicationCredentials,
        layout: StoreLayout,
        account: AccountRecord,
        storage: AdapterStorage,
        cancellation: ActorCancellation,
    ) -> Result<ConnectedActorSession> {
        let workers = NonZeroUsize::new(1).expect("one Telegram actor worker is non-zero");
        let dispatcher = DispatcherBuilder::new()
            .worker_threads(workers)
            .thread_names(|_| "intuigram-telegram".to_owned())
            .build()
            .context(StartActorClusterSnafu)?;
        let cluster = Cluster::from_dispatcher(dispatcher);
        let (startup_tx, startup_rx) = flume::bounded(1);
        let (event_tx, event_rx) = flume::bounded(EFFECT_CAPACITY);
        let spawned = cluster
            .spawn(
                || TelegramActor,
                ActorArguments {
                    credentials,
                    layout,
                    account,
                    storage,
                    startup: startup_tx,
                    events: event_tx,
                    cancellation: cancellation.clone(),
                },
            )
            .capacity(NonZeroUsize::new(EFFECT_CAPACITY).expect("effect capacity is non-zero"))
            .await;
        let (mailbox, handle) = match spawned {
            Ok(actor) => actor,
            Err(source) => {
                let error = spawn_error(source);
                cluster.join().await.context(JoinActorClusterSnafu)?;
                return Err(error);
            }
        };
        let startup = startup_rx
            .recv_async()
            .await
            .map_err(|_| Error::TelegramActorStartupClosed)?;
        Ok(ConnectedActorSession {
            backend: Self {
                owner: Rc::new(ActorOwner {
                    mailbox,
                    handle: RefCell::new(Some(handle)),
                    cluster: RefCell::new(Some(cluster)),
                    cancellation,
                    store: startup.store,
                    local: RefCell::new(super::local_effect::State::new(
                        startup.downloads,
                        startup.media_cache,
                    )),
                }),
            },
            events: ActorEvents::new(event_rx),
            peers: startup.peers,
            bootstrap: startup.bootstrap,
        })
    }
}
