use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub(in crate::application::outbox) enum ScheduledDelivery {
    At(i32),
    WhenOnline,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    rename_all = "snake_case",
    tag = "kind",
    content = "data"
)]
pub(in crate::application::outbox) enum ScheduledCommand {
    Create {
        delivery: ScheduledDelivery,
        text: String,
    },

    Edit {
        message_id: i32,
        text: String,
    },

    Reschedule {
        message_id: i32,
        delivery: ScheduledDelivery,
    },

    Delete {
        message_id: i32,
    },

    SendNow {
        message_id: i32,
    },
}
