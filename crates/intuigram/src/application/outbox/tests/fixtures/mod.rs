mod mutation;
mod scheduled;
mod send;

pub(super) use mutation::mutation_commands;
pub(super) use scheduled::scheduled_commands;
pub(super) use send::send_commands;

use super::super::model::shared::{TextEntity, TextEntityKind};
use super::super::model::{Command, Destination, PreparedCommand};

fn prepared(random_id: Option<i64>, command: Command) -> PreparedCommand {
    PreparedCommand::new(
        Destination {
            chat_id: -100_123,
            thread_root: Some(71),
            saved_peer: Some(-72),
        },
        random_id,
        command,
    )
}

fn text_entities() -> Vec<TextEntity> {
    [
        TextEntityKind::Bold,
        TextEntityKind::Italic,
        TextEntityKind::Underline,
        TextEntityKind::Strike,
        TextEntityKind::Code,
        TextEntityKind::Pre {
            language: Some("rust".to_owned()),
        },
        TextEntityKind::Spoiler,
        TextEntityKind::Url,
        TextEntityKind::TextUrl {
            url: "https://example.com/?q=🚀".to_owned(),
        },
        TextEntityKind::Semantic,
        TextEntityKind::CustomEmoji {
            document_id: i64::MAX,
        },
    ]
    .into_iter()
    .enumerate()
    .map(|(index, kind)| TextEntity {
        offset: u32::try_from(index).expect("fixture index fits u32"),
        length: u32::try_from(index + 1).expect("fixture length fits u32"),
        kind,
    })
    .collect()
}
