use std::collections::HashMap;

use intuigram_app::SavedDialogDraftView;

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
    let (pinned, unread, unread_mark, draft) = match &dialog {
        tl::enums::SavedDialog::Dialog(dialog) => (dialog.pinned, 0, false, None),
        tl::enums::SavedDialog::MonoForumDialog(dialog) => (
            false,
            u32::try_from(dialog.unread_count.max(0)).unwrap_or(0),
            dialog.unread_mark,
            dialog.draft.clone().and_then(normalize_saved_dialog_draft),
        ),
    };
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
        unread,
        unread_mark,
        pinned,
        top_message: MessageId(i64::from(dialog.top_message())),
        draft,
    }
}

fn normalize_saved_dialog_draft(draft: tl::enums::DraftMessage) -> Option<SavedDialogDraftView> {
    let tl::enums::DraftMessage::Message(draft) = draft else {
        return None;
    };
    Some(SavedDialogDraftView {
        text: draft.message,
        reply_to: draft.reply_to.and_then(|reply| match reply {
            tl::enums::InputReplyTo::Message(reply) => {
                Some(MessageId(i64::from(reply.reply_to_msg_id)))
            }
            tl::enums::InputReplyTo::Story(_) | tl::enums::InputReplyTo::MonoForum(_) => None,
        }),
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monoforum_dialog_preserves_unread_and_draft_state() {
        let peer = ChatId(42);
        let dialog: tl::enums::SavedDialog = tl::types::MonoForumDialog {
            unread_mark: true,
            nopaid_messages_exception: false,
            peer: tl::types::PeerUser { user_id: peer.0 }.into(),
            top_message: 99,
            read_inbox_max_id: 95,
            read_outbox_max_id: 98,
            unread_count: 4,
            unread_reactions_count: 1,
            draft: Some(
                tl::types::DraftMessage {
                    no_webpage: false,
                    invert_media: false,
                    reply_to: Some(
                        tl::types::InputReplyToMessage {
                            reply_to_msg_id: 96,
                            top_msg_id: None,
                            reply_to_peer_id: None,
                            quote_text: None,
                            quote_entities: None,
                            quote_offset: None,
                            monoforum_peer_id: None,
                            todo_item_id: None,
                            poll_option: None,
                        }
                        .into(),
                    ),
                    message: "follow up".to_owned(),
                    entities: None,
                    media: None,
                    date: 1_700_000_000,
                    effect: None,
                    suggested_post: None,
                    rich_message: None,
                }
                .into(),
            ),
        }
        .into();
        let names = HashMap::from([(peer, "Ada".to_owned())]);

        let normalized = normalize_saved_dialog(dialog, None, &names);

        assert_eq!(normalized.peer, peer);
        assert_eq!(normalized.title, "Ada");
        assert_eq!(normalized.unread, 4);
        assert!(normalized.unread_mark);
        assert!(!normalized.pinned);
        assert_eq!(normalized.top_message, MessageId(99));
        let draft = normalized.draft.expect("monoforum Draft should normalize");
        assert_eq!(draft.text, "follow up");
        assert_eq!(draft.reply_to, Some(MessageId(96)));
    }
}
