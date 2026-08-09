use std::collections::HashMap;

use super::*;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SavedDialogOffset {
    pub(super) date: i32,
    pub(super) message: i32,
    pub(super) peer: tl::enums::InputPeer,
}

impl Default for SavedDialogOffset {
    fn default() -> Self {
        Self {
            date: 0,
            message: 0,
            peer: tl::enums::InputPeer::Empty,
        }
    }
}

pub(super) struct SavedDialogPage {
    pub(super) total: Option<usize>,
    pub(super) dialogs: Vec<tl::enums::SavedDialog>,
    pub(super) messages: Vec<tl::enums::Message>,
    pub(super) chats: Vec<tl::enums::Chat>,
    pub(super) users: Vec<tl::enums::User>,
}

impl SavedDialogPage {
    pub(super) fn from_response(response: tl::enums::messages::SavedDialogs) -> Self {
        match response {
            tl::enums::messages::SavedDialogs::Dialogs(page) => Self {
                total: Some(page.dialogs.len()),
                dialogs: page.dialogs,
                messages: page.messages,
                chats: page.chats,
                users: page.users,
            },
            tl::enums::messages::SavedDialogs::Slice(page) => Self {
                total: usize::try_from(page.count).ok(),
                dialogs: page.dialogs,
                messages: page.messages,
                chats: page.chats,
                users: page.users,
            },
            tl::enums::messages::SavedDialogs::NotModified(_) => Self {
                total: Some(0),
                dialogs: Vec::new(),
                messages: Vec::new(),
                chats: Vec::new(),
                users: Vec::new(),
            },
        }
    }
}

pub(super) fn saved_dialog_offset(
    dialog: &tl::enums::SavedDialog,
    messages: &HashMap<i32, &tl::enums::Message>,
    peers: &PeerDirectory,
) -> Result<SavedDialogOffset> {
    let message = dialog.top_message();
    let date = messages
        .get(&message)
        .map_or(0, |message| message_date(message));
    Ok(SavedDialogOffset {
        date,
        message,
        peer: peers.resolve(marked_peer_id(&dialog.peer()))?,
    })
}

pub(super) fn normalize_saved_dialog(
    dialog: tl::enums::SavedDialog,
    top: Option<&&tl::enums::Message>,
    names: &HashMap<ChatId, String>,
) -> SavedDialogView {
    let peer = marked_peer_id(&dialog.peer());
    let (preview, timestamp) = top.map_or_else(
        || (String::new(), String::new()),
        |message| {
            let (preview, _, _, timestamp) = dialog_message_summary(message, names);
            (preview, timestamp)
        },
    );
    SavedDialogView {
        peer,
        title: names
            .get(&peer)
            .cloned()
            .unwrap_or_else(|| "Unknown peer".to_owned()),
        preview,
        timestamp,
        pinned: matches!(dialog, tl::enums::SavedDialog::Dialog(ref dialog) if dialog.pinned),
        top_message: MessageId(i64::from(dialog.top_message())),
    }
}

pub(super) fn message_id(message: &tl::enums::Message) -> i32 {
    match message {
        tl::enums::Message::Empty(message) => message.id,
        tl::enums::Message::Message(message) => message.id,
        tl::enums::Message::Service(message) => message.id,
    }
}

fn message_date(message: &tl::enums::Message) -> i32 {
    match message {
        tl::enums::Message::Empty(_) => 0,
        tl::enums::Message::Message(message) => message.date,
        tl::enums::Message::Service(message) => message.date,
    }
}
