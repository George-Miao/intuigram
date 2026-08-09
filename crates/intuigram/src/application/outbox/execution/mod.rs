use intuigram_app::AdapterEvent;
use intuigram_store::{OutboxCompletion, OutboxPayload, OutboxRecord};
use intuigram_telegram::{LiveEvent, RetryDisposition};
use snafu::{ResultExt, Snafu};

use super::super::Backend;
use super::model::{Command, PreparedCommand};

mod conversion;
mod interaction;
mod location;
mod media;
mod message;
mod mutation;
mod scheduled;
mod send;
#[cfg(test)]
mod tests;

#[derive(Debug, Snafu)]
pub(in crate::application) enum Error {
    #[snafu(display("durable Outbox command is corrupt"))]
    Decode { source: super::codec::Error },

    #[snafu(display("durable Outbox command is inconsistent: {reason}"))]
    Invalid { reason: &'static str },

    #[snafu(display("durable Telegram operation failed"))]
    Telegram { source: intuigram_telegram::Error },
}

pub(in crate::application) type Result<T> = std::result::Result<T, Error>;

pub(in crate::application) struct Success {
    pub(in crate::application) completion: OutboxCompletion,
    pub(in crate::application) event: Option<AdapterEvent>,
    pub(in crate::application) update: Option<LiveEvent>,
}

impl Error {
    pub(in crate::application) fn retry_disposition(&self) -> RetryDisposition {
        match self {
            Self::Telegram { source } => source.retry_disposition(),
            Self::Decode { .. } | Self::Invalid { .. } => RetryDisposition::DoNotRetry,
        }
    }
}

pub(in crate::application) async fn execute(
    backend: &mut Backend,
    record: &OutboxRecord,
) -> Result<Success> {
    let OutboxPayload::V1(payload) = &record.payload;
    let prepared = super::codec::decode(&payload.content).context(DecodeSnafu)?;
    validate(payload, &prepared)?;
    match prepared.command() {
        Command::Text(send) => send::text(backend, &prepared, send, &record.media).await,
        Command::Poll(send) => send::poll(backend, &prepared, send).await,
        Command::Library(send) => media::library(backend, &prepared, send).await,
        Command::Contact(send) => media::contact(backend, &prepared, send).await,
        Command::File(send) | Command::Recording(send) => {
            media::upload(backend, &prepared, send, &record.media).await
        }
        Command::StaticLocation(send) => location::location(backend, &prepared, send).await,
        Command::Venue(send) => location::venue(backend, &prepared, send).await,
        Command::Scheduled(command) => scheduled::execute(backend, &prepared, command).await,
        Command::Mutation(command) => {
            mutation::execute(backend, &prepared, command, &record.media).await
        }
    }
}

fn validate(payload: &intuigram_store::OutboxPayloadV1, command: &PreparedCommand) -> Result<()> {
    let destination = command.destination();
    if destination.chat_id != payload.chat_id
        || destination.thread_root != payload.thread_root
        || destination.saved_peer != payload.saved_peer
        || command.local_message_id() != payload.local_message_id
        || command.random_id() != Some(payload.random_id)
    {
        return InvalidSnafu {
            reason: "stored envelope does not match its semantic command",
        }
        .fail();
    }
    Ok(())
}

fn telegram<T>(result: intuigram_telegram::Result<T>) -> Result<T> {
    result.context(TelegramSnafu)
}
