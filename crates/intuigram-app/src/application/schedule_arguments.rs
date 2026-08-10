use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use super::*;

pub(super) fn parse_scheduled_maintenance(
    arguments: &mut impl Iterator<Item = String>,
    argument: &str,
) -> Result<ScheduledMaintenance> {
    match argument {
        "--schedule-message" => Ok(ScheduledMaintenance::Create {
            chat: chat(argument, next_argument(arguments, argument)?)?,
            delivery: delivery(argument, next_argument(arguments, argument)?)?,
            text: next_argument(arguments, argument)?,
        }),
        "--scheduled-list" => Ok(ScheduledMaintenance::List {
            chat: chat(argument, next_argument(arguments, argument)?)?,
        }),
        "--scheduled-edit" => Ok(ScheduledMaintenance::Edit {
            chat: chat(argument, next_argument(arguments, argument)?)?,
            message: message(argument, next_argument(arguments, argument)?)?,
            text: next_argument(arguments, argument)?,
        }),
        "--scheduled-reschedule" => Ok(ScheduledMaintenance::Reschedule {
            chat: chat(argument, next_argument(arguments, argument)?)?,
            message: message(argument, next_argument(arguments, argument)?)?,
            delivery: delivery(argument, next_argument(arguments, argument)?)?,
        }),
        "--scheduled-delete" => Ok(ScheduledMaintenance::Delete {
            chat: chat(argument, next_argument(arguments, argument)?)?,
            message: message(argument, next_argument(arguments, argument)?)?,
        }),
        "--scheduled-send-now" => Ok(ScheduledMaintenance::SendNow {
            chat: chat(argument, next_argument(arguments, argument)?)?,
            message: message(argument, next_argument(arguments, argument)?)?,
        }),
        _ => UnknownArgumentSnafu {
            argument: argument.to_owned(),
        }
        .fail(),
    }
}

fn chat(argument: &str, value: String) -> Result<ChatId> {
    value
        .parse::<i64>()
        .ok()
        .filter(|id| *id != 0)
        .map(ChatId)
        .ok_or_else(|| invalid(argument, value))
}

fn message(argument: &str, value: String) -> Result<i32> {
    value
        .parse::<i32>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| invalid(argument, value))
}

fn delivery(argument: &str, value: String) -> Result<ScheduledDelivery> {
    if value == "online" {
        return Ok(ScheduledDelivery::WhenOnline);
    }
    OffsetDateTime::parse(&value, &Rfc3339)
        .ok()
        .and_then(|date| i32::try_from(date.unix_timestamp()).ok())
        .map(ScheduledDelivery::At)
        .ok_or_else(|| invalid(argument, value))
}

fn invalid(argument: &str, value: String) -> Error {
    Error::InvalidArgumentValue {
        argument: argument.to_owned(),
        value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_time_requires_an_explicit_utc_offset() {
        let mut arguments = [
            "7".to_owned(),
            "2030-06-01T09:30:00+08:00".to_owned(),
            "hello".to_owned(),
        ]
        .into_iter();
        let command = parse_scheduled_maintenance(&mut arguments, "--schedule-message")
            .expect("an explicitly offset schedule should parse");
        assert!(matches!(
            command,
            ScheduledMaintenance::Create {
                chat: ChatId(7),
                delivery: ScheduledDelivery::At(1_906_507_800),
                ref text,
            } if text == "hello"
        ));

        let mut local = [
            "7".to_owned(),
            "2030-06-01T09:30:00".to_owned(),
            "hello".to_owned(),
        ]
        .into_iter();
        assert!(parse_scheduled_maintenance(&mut local, "--schedule-message").is_err());

        let mut online = ["7".to_owned(), "online".to_owned(), "hello".to_owned()].into_iter();
        assert!(matches!(
            parse_scheduled_maintenance(&mut online, "--schedule-message"),
            Ok(ScheduledMaintenance::Create {
                delivery: ScheduledDelivery::WhenOnline,
                ..
            })
        ));
    }
}
