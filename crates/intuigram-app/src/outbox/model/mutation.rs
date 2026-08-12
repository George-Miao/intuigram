use serde::{Deserialize, Serialize};

use super::shared::{PreparedAttachment, TextEntity};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    rename_all = "snake_case",
    tag = "kind",
    content = "data"
)]
pub(in crate::outbox) enum MutationCommand {
    Edit {
        message_id: i64,
        text: String,
        entities: Vec<TextEntity>,
        attachments: Vec<PreparedAttachment>,
    },

    Delete {
        message_ids: Vec<i64>,
    },

    Forward {
        source_chat_id: i64,
        message_ids: Vec<i64>,
    },

    Reaction {
        message_id: i64,
        reaction: String,
    },

    Pin {
        message_id: i64,
        pinned: bool,
    },

    Vote {
        message_id: i64,
        options: Vec<u32>,
    },

    ToggleTodo {
        message_id: i64,
        item_id: i32,
        completed: bool,
    },

    AppendTodo {
        message_id: i64,
        title: String,
    },
}
