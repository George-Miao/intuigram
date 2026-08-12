use std::collections::VecDeque;
use std::convert::Infallible;
use std::num::NonZeroUsize;
use std::task::Poll;

use compio::actor::mailbox::DeliverError;
use compio::actor::{Actor, ActorExit, Cluster, Handler, Mailbox};
use compio::dispatcher::DispatcherBuilder;
use compio::runtime::ResumeUnwind;

use super::super::runtime::{WorkerAdapterEvents, WorkerBatch};
use super::super::{AdapterBatch, Error, Result};
use super::cancellation::{ActorCancellation, until_cancelled};
use super::driver::{DriverStop, SessionEvent, run_driver};

struct RuntimeProbe;

struct SaturatedActor;

struct SaturatedState {
    entered: flume::Sender<()>,
    release: flume::Receiver<()>,
}

struct DeliveryEvent {
    batches: VecDeque<WorkerBatch>,
}

struct CancelableActor;

struct CancelableState {
    cancellation: ActorCancellation,
    entered: flume::Sender<()>,
}

impl WorkerAdapterEvents for DeliveryEvent {
    fn poll_worker_event(&mut self, _cx: &mut std::task::Context<'_>) -> Poll<Result<WorkerBatch>> {
        self.batches
            .pop_front()
            .map_or(Poll::Pending, |batch| Poll::Ready(Ok(batch)))
    }

    fn close(&mut self) {}
}

impl Actor for RuntimeProbe {
    type Arguments = flume::Sender<bool>;
    type Error = Infallible;
    type State = ();

    async fn pre_start(
        &self,
        _myself: &Mailbox<Self>,
        result: Self::Arguments,
    ) -> std::result::Result<Self::State, Self::Error> {
        result
            .send(compio::runtime::Runtime::try_current().is_some())
            .ok();
        Ok(())
    }
}

impl Actor for SaturatedActor {
    type Arguments = SaturatedState;
    type Error = Infallible;
    type State = SaturatedState;

    async fn pre_start(
        &self,
        _myself: &Mailbox<Self>,
        state: Self::Arguments,
    ) -> std::result::Result<Self::State, Self::Error> {
        Ok(state)
    }
}

impl Handler<u8> for SaturatedActor {
    async fn handle(
        &self,
        _myself: &Mailbox<Self>,
        message: u8,
        state: &mut Self::State,
    ) -> std::result::Result<(), Self::Error> {
        if message == 0 {
            state.entered.send(()).ok();
            state.release.recv_async().await.ok();
        }
        Ok(())
    }
}

impl Actor for CancelableActor {
    type Arguments = CancelableState;
    type Error = Error;
    type State = CancelableState;

    async fn pre_start(
        &self,
        _myself: &Mailbox<Self>,
        state: Self::Arguments,
    ) -> std::result::Result<Self::State, Self::Error> {
        Ok(state)
    }
}

impl Handler<u8> for CancelableActor {
    async fn handle(
        &self,
        _myself: &Mailbox<Self>,
        _message: u8,
        state: &mut Self::State,
    ) -> std::result::Result<(), Self::Error> {
        state.entered.send(()).ok();
        until_cancelled(std::future::pending(), &state.cancellation).await
    }
}

#[test]
fn cluster_workers_enter_the_same_compio_runtime_generation() {
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");
    runtime.block_on(async {
        let workers = NonZeroUsize::new(1).expect("one worker is non-zero");
        let dispatcher = DispatcherBuilder::new()
            .worker_threads(workers)
            .build()
            .expect("test dispatcher should initialize");
        let cluster = Cluster::from_dispatcher(dispatcher);
        let (result_tx, result_rx) = flume::bounded(1);
        let (mailbox, handle) = cluster
            .spawn(|| RuntimeProbe, result_tx)
            .await
            .expect("probe actor should start");

        assert!(
            result_rx
                .recv_async()
                .await
                .expect("probe actor should report its runtime")
        );

        mailbox.stop();
        assert!(matches!(
            handle.await.expect("probe actor should return its exit"),
            ActorExit::Stopped
        ));
        cluster.join().await.expect("test cluster should stop");
    });
}

#[test]
fn actor_mailbox_reports_saturation_without_blocking_the_caller() {
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");
    runtime.block_on(async {
        let workers = NonZeroUsize::new(1).expect("one worker is non-zero");
        let dispatcher = DispatcherBuilder::new()
            .worker_threads(workers)
            .build()
            .expect("test dispatcher should initialize");
        let cluster = Cluster::from_dispatcher(dispatcher);
        let (entered_tx, entered_rx) = flume::bounded(1);
        let (release_tx, release_rx) = flume::bounded(1);
        let (mailbox, handle) = cluster
            .spawn(
                || SaturatedActor,
                SaturatedState {
                    entered: entered_tx,
                    release: release_rx,
                },
            )
            .with_capacity(NonZeroUsize::new(1).expect("one message is non-zero"))
            .await
            .expect("saturation actor should start");

        mailbox.send(0).expect("first message should start");
        entered_rx
            .recv_async()
            .await
            .expect("actor should enter its held handler");
        mailbox
            .send(1)
            .expect("second message should fill the mailbox");
        assert!(matches!(mailbox.send(2), Err(DeliverError::Full(2))));

        release_tx
            .send(())
            .expect("held handler should be released");
        mailbox.stop();
        handle.await.expect("actor should report its exit");
        cluster.join().await.expect("test cluster should stop");
    });
}

#[test]
fn pending_actor_command_is_cancelled_before_cluster_join() {
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");
    runtime.block_on(async {
        let workers = NonZeroUsize::new(1).expect("one worker is non-zero");
        let dispatcher = DispatcherBuilder::new()
            .worker_threads(workers)
            .build()
            .expect("test dispatcher should initialize");
        let cluster = Cluster::from_dispatcher(dispatcher);
        let cancellation = ActorCancellation::default();
        let (entered_tx, entered_rx) = flume::bounded(1);
        let (mailbox, handle) = cluster
            .spawn(
                || CancelableActor,
                CancelableState {
                    cancellation: cancellation.clone(),
                    entered: entered_tx,
                },
            )
            .await
            .expect("cancelable actor should start");

        mailbox.send(0).expect("pending command should be accepted");
        entered_rx
            .recv_async()
            .await
            .expect("pending command should start");
        cancellation.cancel();

        assert!(matches!(
            handle.await.expect("actor should report its exit"),
            ActorExit::Failed(Error::TelegramActorCancelled)
        ));
        cluster.join().await.expect("cancelled cluster should join");
    });
}

#[test]
fn submitted_commit_is_acknowledged_after_driver_delivery() {
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");
    runtime.block_on(async {
        let submitted = super::super::SubmittedUpdates::default();
        let delivered = submitted.push(intuigram_telegram::LiveEvent {
            events: Vec::new(),
            cursors: Vec::new(),
            peers: intuigram_telegram::PeerDirectory::default(),
        });
        let delivered_tx = std::future::poll_fn(|cx| submitted.poll_pop(cx))
            .await
            .expect("delivery completion should be queued")
            .committed;
        let events = DeliveryEvent {
            batches: VecDeque::from([
                WorkerBatch {
                    batch: AdapterBatch {
                        event: Some(intuigram_lib::AdapterEvent::OperationCompleted(
                            "first".to_owned(),
                        )),
                        peers: intuigram_telegram::PeerDirectory::default(),
                    },
                    delivered: None,
                },
                WorkerBatch {
                    batch: AdapterBatch {
                        event: Some(intuigram_lib::AdapterEvent::OperationCompleted(
                            "second".to_owned(),
                        )),
                        peers: intuigram_telegram::PeerDirectory::default(),
                    },
                    delivered: Some(delivered_tx),
                },
            ]),
        };
        let stop = DriverStop::default();
        let (output_tx, output_rx) = flume::bounded(1);
        let driver = compio::runtime::spawn(run_driver(events, stop.clone(), output_tx));

        for expected in ["first", "second"] {
            let SessionEvent::Batch(batch) = output_rx
                .recv_async()
                .await
                .expect("driver should preserve every committed event")
            else {
                panic!("driver unexpectedly failed")
            };
            assert!(matches!(
                batch.event,
                Some(intuigram_lib::AdapterEvent::OperationCompleted(ref value))
                    if value == expected
            ));
        }
        delivered.await.expect("delivery should succeed");

        stop.stop();
        driver
            .await
            .resume_unwind()
            .expect("driver task should stop cleanly");
    });
}
