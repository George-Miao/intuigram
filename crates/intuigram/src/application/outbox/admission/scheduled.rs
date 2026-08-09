use intuigram_app::{ScheduledDeliveryView, ScheduledRequest};
use intuigram_store::OutboxOperation;

use super::super::model::scheduled::{ScheduledCommand, ScheduledDelivery};

pub(super) fn prepare(request: &ScheduledRequest) -> (OutboxOperation, ScheduledCommand) {
    match request {
        ScheduledRequest::Create { delivery, text } => (
            OutboxOperation::Create,
            ScheduledCommand::Create {
                delivery: delivery_view(*delivery),
                text: text.clone(),
            },
        ),
        ScheduledRequest::Edit { message, text } => (
            OutboxOperation::Mutation,
            ScheduledCommand::Edit {
                message_id: message.0,
                text: text.clone(),
            },
        ),
        ScheduledRequest::Reschedule { message, delivery } => (
            OutboxOperation::Mutation,
            ScheduledCommand::Reschedule {
                message_id: message.0,
                delivery: delivery_view(*delivery),
            },
        ),
        ScheduledRequest::Delete { message } => (
            OutboxOperation::Mutation,
            ScheduledCommand::Delete {
                message_id: message.0,
            },
        ),
        ScheduledRequest::SendNow { message } => (
            OutboxOperation::Mutation,
            ScheduledCommand::SendNow {
                message_id: message.0,
            },
        ),
    }
}

const fn delivery_view(value: ScheduledDeliveryView) -> ScheduledDelivery {
    match value {
        ScheduledDeliveryView::At(timestamp) => ScheduledDelivery::At(timestamp),
        ScheduledDeliveryView::WhenOnline => ScheduledDelivery::WhenOnline,
    }
}
