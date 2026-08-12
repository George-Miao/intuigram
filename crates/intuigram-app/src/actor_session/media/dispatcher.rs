use std::collections::VecDeque;

use compio::actor::Reply;
use compio::runtime::{JoinHandle, ResumeUnwind};
use intuigram_lib::{AvatarRef, ChatId, MediaLocator, MessageId, OfflineMediaTarget};

use super::actor::ActorResponse;
use super::cancellation::ActorCancellation;
use super::{EffectCancellation, Error, Result, response};

const MAX_RETAINED_TASKS: usize = 64;

pub(in crate::actor_session) enum MediaOperation {
    Avatar(AvatarRef),
    Preview {
        chat: ChatId,
        message: MessageId,
        locator: Option<MediaLocator>,
    },
    Download {
        chat: ChatId,
        message: MessageId,
        destination: Option<String>,
        locator: Option<MediaLocator>,
    },
    Offline {
        target: OfflineMediaTarget,
        locator: Option<MediaLocator>,
    },
}

struct MediaJob {
    client: intuigram_telegram::MediaClient,
    operation: MediaOperation,
    cancellation: EffectCancellation,
    reply: Reply<ActorResponse>,
}

pub(in crate::actor_session) struct MediaDispatcher {
    sender: Option<flume::Sender<MediaJob>>,
    driver: Option<JoinHandle<()>>,
}

impl MediaDispatcher {
    pub(in crate::actor_session) fn start(cancellation: ActorCancellation) -> Self {
        let (sender, receiver) = flume::bounded(MAX_RETAINED_TASKS);
        let driver = compio::runtime::spawn(run(receiver, cancellation));
        Self {
            sender: Some(sender),
            driver: Some(driver),
        }
    }

    pub(in crate::actor_session) fn submit(
        &self,
        client: intuigram_telegram::MediaClient,
        operation: MediaOperation,
        cancellation: EffectCancellation,
        reply: Reply<ActorResponse>,
    ) {
        let job = MediaJob {
            client,
            operation,
            cancellation,
            reply,
        };
        let Some(sender) = &self.sender else {
            job.reply
                .reply(ActorResponse::Failed(Box::new(
                    Error::TelegramActorMailboxClosed,
                )))
                .ok();
            return;
        };
        if let Err(error) = sender.try_send(job) {
            let job = error.into_inner();
            job.reply
                .reply(ActorResponse::Failed(Box::new(
                    Error::TelegramActorMailboxFull,
                )))
                .ok();
        }
    }

    pub(in crate::actor_session) async fn stop(&mut self) -> Result<()> {
        self.sender.take();
        if let Some(driver) = self.driver.take() {
            driver
                .await
                .resume_unwind()
                .ok_or(Error::TelegramActorDriverCancelled)?;
        }
        Ok(())
    }
}

async fn run(receiver: flume::Receiver<MediaJob>, cancellation: ActorCancellation) {
    let mut tasks = VecDeque::new();
    while let Ok(job) = receiver.recv_async().await {
        let cancellation = cancellation.clone();
        tasks.push_back(compio::runtime::spawn(execute(job, cancellation)));
        if tasks.len() >= MAX_RETAINED_TASKS
            && let Some(oldest) = tasks.pop_front()
        {
            oldest.await.resume_unwind();
        }
    }
    while let Some(task) = tasks.pop_front() {
        task.await.resume_unwind();
    }
}

async fn execute(mut job: MediaJob, cancellation: ActorCancellation) {
    let response = match job.operation {
        MediaOperation::Avatar(avatar) => {
            response::avatar(&mut job.client, &cancellation, &job.cancellation, avatar).await
        }
        MediaOperation::Preview {
            chat,
            message,
            locator,
        } => {
            response::preview(
                &mut job.client,
                &cancellation,
                &job.cancellation,
                chat,
                message,
                locator,
            )
            .await
        }
        MediaOperation::Download {
            chat,
            message,
            destination,
            locator,
        } => {
            response::download(
                &mut job.client,
                &cancellation,
                &job.cancellation,
                chat,
                message,
                destination,
                locator,
            )
            .await
        }
        MediaOperation::Offline { target, locator } => {
            response::offline(
                &mut job.client,
                &cancellation,
                &job.cancellation,
                target,
                locator,
            )
            .await
        }
    };
    job.reply.reply(response).ok();
}
