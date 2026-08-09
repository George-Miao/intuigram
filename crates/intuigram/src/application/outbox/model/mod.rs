use serde::{Deserialize, Serialize};

pub(super) mod mutation;
pub(super) mod scheduled;
pub(super) mod send;
pub(super) mod shared;

use mutation::MutationCommand;
use scheduled::ScheduledCommand;
use send::{Contact, LibraryMedia, MessageSend, Poll, TextMessage, Venue};
use shared::{GeoPoint, PreparedMedia};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Destination {
    pub(super) chat_id: i64,
    pub(super) thread_root: Option<i64>,
    pub(super) saved_peer: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PreparedCommand {
    destination: Destination,
    random_id: Option<i64>,
    command: Command,
}

impl PreparedCommand {
    pub(super) const fn new(
        destination: Destination,
        random_id: Option<i64>,
        command: Command,
    ) -> Self {
        Self {
            destination,
            random_id,
            command,
        }
    }

    pub(super) const fn destination(&self) -> Destination {
        self.destination
    }

    pub(super) const fn random_id(&self) -> Option<i64> {
        self.random_id
    }

    pub(super) const fn command(&self) -> &Command {
        &self.command
    }

    pub(super) const fn local_message_id(&self) -> Option<i64> {
        match &self.command {
            Command::Text(send) => Some(send.local_message_id),
            Command::Poll(send) => Some(send.local_message_id),
            Command::Library(send) => Some(send.local_message_id),
            Command::Contact(send) => Some(send.local_message_id),
            Command::File(send) => Some(send.local_message_id),
            Command::Recording(send) => Some(send.local_message_id),
            Command::StaticLocation(send) => Some(send.local_message_id),
            Command::Venue(send) => Some(send.local_message_id),
            Command::Scheduled(_) | Command::Mutation(_) => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    rename_all = "snake_case",
    tag = "kind",
    content = "data"
)]
pub(super) enum Command {
    Text(MessageSend<TextMessage>),
    Poll(MessageSend<Poll>),
    Library(MessageSend<LibraryMedia>),
    Contact(MessageSend<Contact>),
    File(MessageSend<PreparedMedia>),
    Recording(MessageSend<PreparedMedia>),
    StaticLocation(MessageSend<GeoPoint>),
    Venue(MessageSend<Venue>),
    Scheduled(ScheduledCommand),
    Mutation(MutationCommand),
}
