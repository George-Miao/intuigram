use super::super::super::model::mutation::MutationCommand;
use super::super::super::model::shared::{AttachmentKind, MediaPosition, PreparedAttachment};
use super::super::super::model::{Command, PreparedCommand};
use super::{prepared, text_entities};

pub(in crate::application::outbox::tests) fn mutation_commands() -> Vec<PreparedCommand> {
    vec![
        prepared(
            Some(14),
            Command::Mutation(MutationCommand::Edit {
                message_id: 51,
                text: "replacement".to_owned(),
                entities: text_entities(),
                attachments: vec![PreparedAttachment::new(
                    MediaPosition(9),
                    AttachmentKind::Video,
                )],
            }),
        ),
        prepared(
            None,
            Command::Mutation(MutationCommand::Delete {
                message_ids: vec![52, 53],
            }),
        ),
        prepared(
            Some(15),
            Command::Mutation(MutationCommand::Forward {
                source_chat_id: -200,
                message_ids: vec![54, 55],
            }),
        ),
        prepared(
            None,
            Command::Mutation(MutationCommand::Reaction {
                message_id: 56,
                reaction: "🧊".to_owned(),
            }),
        ),
        prepared(
            None,
            Command::Mutation(MutationCommand::Pin {
                message_id: 57,
                pinned: true,
            }),
        ),
        prepared(
            None,
            Command::Mutation(MutationCommand::Vote {
                message_id: 58,
                options: vec![0, u32::MAX],
            }),
        ),
        prepared(
            None,
            Command::Mutation(MutationCommand::ToggleTodo {
                message_id: 59,
                item_id: i32::MIN,
                completed: false,
            }),
        ),
        prepared(
            None,
            Command::Mutation(MutationCommand::AppendTodo {
                message_id: 60,
                title: "ship it".to_owned(),
            }),
        ),
    ]
}
