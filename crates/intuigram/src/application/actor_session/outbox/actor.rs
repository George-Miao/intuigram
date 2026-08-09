use compio_actor::{Call, Handler, Mailbox};
use intuigram_store::OutboxRecord;

use super::super::super::{Error, Result, SubmittedUpdates};
use super::super::actor::TelegramActor;
use super::super::cancellation::until_cancelled_result;

pub(super) struct ExecuteOutbox(pub(super) OutboxRecord);

pub(super) enum OutboxResponse {
    Complete(Box<super::super::super::outbox::execution::Success>),
    ExecutionFailed(Box<super::super::super::outbox::execution::Error>),
    Failed(Box<Error>),
    Cancelled,
}

impl Handler<Call<ExecuteOutbox, OutboxResponse>> for TelegramActor {
    async fn handle(
        &self,
        _myself: &Mailbox<Self>,
        call: Call<ExecuteOutbox, OutboxResponse>,
        state: &mut Self::State,
    ) -> Result<()> {
        let (request, reply) = call.into_parts();
        let result = until_cancelled_result(
            super::super::super::outbox::execution::execute(&mut state.backend, &request.0),
            &state.cancellation,
        )
        .await;
        let response = match result {
            Ok(Ok(success)) => response(success, &state.submitted).await,
            Ok(Err(error)) => OutboxResponse::ExecutionFailed(Box::new(error)),
            Err(Error::TelegramActorCancelled) => OutboxResponse::Cancelled,
            Err(error) => OutboxResponse::Failed(Box::new(error)),
        };
        reply.reply(response).ok();
        Ok(())
    }
}

async fn response(
    mut success: super::super::super::outbox::execution::Success,
    submitted: &SubmittedUpdates,
) -> OutboxResponse {
    let Some(update) = success.update.take() else {
        return OutboxResponse::Complete(Box::new(success));
    };
    match submitted.push(update).await {
        Ok(()) => OutboxResponse::Complete(Box::new(success)),
        Err(error) => OutboxResponse::Failed(error),
    }
}
