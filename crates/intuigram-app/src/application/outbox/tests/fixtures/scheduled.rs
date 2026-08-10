use super::super::super::model::scheduled::{ScheduledCommand, ScheduledDelivery};
use super::super::super::model::{Command, PreparedCommand};
use super::prepared;

pub(in crate::application::outbox::tests) fn scheduled_commands() -> Vec<PreparedCommand> {
    vec![
        prepared(
            Some(9),
            Command::Scheduled(ScheduledCommand::Create {
                delivery: ScheduledDelivery::At(i32::MAX),
                text: "later".to_owned(),
            }),
        ),
        prepared(
            Some(10),
            Command::Scheduled(ScheduledCommand::Edit {
                message_id: -1,
                text: "changed".to_owned(),
            }),
        ),
        prepared(
            Some(11),
            Command::Scheduled(ScheduledCommand::Reschedule {
                message_id: 2,
                delivery: ScheduledDelivery::WhenOnline,
            }),
        ),
        prepared(
            Some(12),
            Command::Scheduled(ScheduledCommand::Delete { message_id: 3 }),
        ),
        prepared(
            Some(13),
            Command::Scheduled(ScheduledCommand::SendNow { message_id: 4 }),
        ),
    ]
}
