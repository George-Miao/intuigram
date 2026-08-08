use compio_actor::cluster::SpawnError;
use compio_actor::mailbox::{CallError, DeliverError};

use super::Error;

pub(super) fn spawn_error(error: SpawnError<Error>) -> Error {
    match error {
        SpawnError::Unavailable => Error::TelegramActorUnavailable,
        SpawnError::NameTaken { name } => Error::TelegramActorNameTaken {
            name: name.into_owned(),
        },
        SpawnError::Start { error } => error,
        SpawnError::WorkerStopped => Error::TelegramActorWorkerStopped,
        SpawnError::SupervisorStopped => Error::TelegramActorSupervisorStopped,
    }
}

pub(super) fn call_error<M>(error: CallError<M>) -> Error
where
    M: Send + 'static,
{
    match error {
        CallError::Full { .. } => Error::TelegramActorMailboxFull,
        CallError::Closed { .. } => Error::TelegramActorMailboxClosed,
        CallError::NoReply => Error::TelegramActorNoReply,
    }
}

pub(super) fn deliver_error<M>(error: DeliverError<M>) -> Error
where
    M: Send + 'static,
{
    match error {
        DeliverError::Full { .. } => Error::TelegramActorMailboxFull,
        DeliverError::Closed { .. } => Error::TelegramActorMailboxClosed,
    }
}
