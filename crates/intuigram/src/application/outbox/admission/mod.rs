use intuigram::OperationStamp;
use intuigram_app::Effect;
use intuigram_store::OutboxAdmission;
use intuigram_telegram::MediaLibraryEntry;
use snafu::Snafu;

use super::super::{AttachmentPayload, PreparedRichMedia};

mod conversion;
mod input;
mod media;
mod message;
mod mutation;
mod scheduled;

#[derive(Debug, Snafu)]
pub(in crate::application) enum Error {
    #[snafu(display("durable Outbox input is incomplete: {reason}"))]
    Incomplete { reason: &'static str },

    #[snafu(display("durable Outbox content exceeds its stable numeric representation"))]
    NumericOverflow,

    #[snafu(display("durable Outbox command could not be encoded"))]
    Encode { source: super::codec::Error },
}

type Result<T> = std::result::Result<T, Error>;

pub(in crate::application) const fn handles(effect: &Effect) -> bool {
    matches!(
        effect,
        Effect::SendStaticLocation { .. }
            | Effect::SendVenue { .. }
            | Effect::SendLibraryMedia { .. }
            | Effect::SendRichMediaFile { .. }
            | Effect::RecordRichMedia { .. }
            | Effect::SendContact { .. }
            | Effect::ScheduledOperation { .. }
            | Effect::SendMessage { .. }
            | Effect::SendPoll { .. }
            | Effect::EditMessage { .. }
            | Effect::DeleteMessages { .. }
            | Effect::ForwardMessages { .. }
            | Effect::ReactMessage { .. }
            | Effect::SetMessagePinned { .. }
            | Effect::VotePoll { .. }
            | Effect::ToggleTodoItem { .. }
            | Effect::AppendTodoItem { .. }
    )
}

pub(in crate::application) fn prepare(
    effect: &Effect,
    stamp: OperationStamp,
    attachments: Vec<(intuigram_app::AttachmentId, AttachmentPayload)>,
    rich_media: Option<PreparedRichMedia>,
    library: Option<MediaLibraryEntry>,
) -> Result<OutboxAdmission> {
    let inputs = input::PreparedInputs::new(attachments, rich_media, library);
    match effect {
        Effect::SendMessage { .. }
        | Effect::SendPoll { .. }
        | Effect::SendLibraryMedia { .. }
        | Effect::SendRichMediaFile { .. }
        | Effect::RecordRichMedia { .. }
        | Effect::SendContact { .. }
        | Effect::SendStaticLocation { .. }
        | Effect::SendVenue { .. } => message::prepare(effect, stamp, inputs),
        Effect::ScheduledOperation { .. }
        | Effect::EditMessage { .. }
        | Effect::DeleteMessages { .. }
        | Effect::ForwardMessages { .. }
        | Effect::ReactMessage { .. }
        | Effect::SetMessagePinned { .. }
        | Effect::VotePoll { .. }
        | Effect::ToggleTodoItem { .. }
        | Effect::AppendTodoItem { .. } => mutation::prepare(effect, stamp, inputs),
        _ => Err(Error::Incomplete {
            reason: "effect is not a durable outbound operation",
        }),
    }
}
